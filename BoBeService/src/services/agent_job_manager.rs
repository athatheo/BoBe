//! AgentJobManager — manages coding agent subprocess lifecycle.
//!
//! Launches coding agents (Claude Code, Aider, OpenCode) as headless subprocesses.
//! Handles spawning, monitoring, output collection, and cleanup.
//!
//! Concurrency: singleton, protected by async lock, max_concurrent limit.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};
use tracing::{error, info, warn};

use crate::db::AgentJobRepository;
use crate::error::AppError;
use crate::models::agent_job::AgentJob;
use crate::models::ids::{AgentJobId, ConversationId};
use crate::tools::mcp::security::{filter_safe_env_vars, validate_subprocess_command_spec};

use super::agent_output_parsers::{AgentJobResult, parse_claude_ndjson, parse_text_output};

const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;
const STDERR_SUMMARY_BYTES: usize = 8 * 1024;

const KILL_GRACE_SECONDS: u64 = 5;

static AGENT_BLOCKED_COMMANDS: LazyLock<Vec<String>> = LazyLock::new(|| {
    [
        "rm",
        "rmdir",
        "dd",
        "mkfs",
        "fdisk",
        "format",
        "del",
        "sudo",
        "su",
        "doas",
        "chmod",
        "chown",
        "chgrp",
        "kill",
        "killall",
        "pkill",
        "reboot",
        "shutdown",
        "halt",
        "init",
        "systemctl",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
});

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct AgentProfileConfig {
    pub(crate) name: String,
    pub(crate) command: String,
    pub(crate) args: Vec<String>,
    #[serde(default = "default_output_format")]
    pub(crate) output_format: String,
    #[serde(default = "default_true")]
    pub(crate) enabled: bool,
    pub(crate) max_runtime_seconds: Option<u64>,
    pub(crate) working_directory: Option<String>,
    pub(crate) env_vars: Option<HashMap<String, String>>,
}

fn default_output_format() -> String {
    "text".into()
}

fn default_true() -> bool {
    true
}

pub(crate) struct AgentJobManager {
    repo: Arc<dyn AgentJobRepository>,
    profiles: HashMap<String, AgentProfileConfig>,
    output_dir: PathBuf,
    max_concurrent: usize,
    max_runtime_seconds: u64,
    slots: Arc<Semaphore>,
    on_job_complete: Mutex<
        Option<Arc<dyn Fn(AgentJob) -> futures::future::BoxFuture<'static, ()> + Send + Sync>>,
    >,
}

impl AgentJobManager {
    pub(crate) fn new(
        repo: Arc<dyn AgentJobRepository>,
        profiles: HashMap<String, AgentProfileConfig>,
        output_dir: PathBuf,
        max_concurrent: usize,
        max_runtime_seconds: u64,
    ) -> Self {
        Self {
            repo,
            profiles,
            output_dir,
            max_concurrent,
            max_runtime_seconds,
            slots: Arc::new(Semaphore::new(max_concurrent)),
            on_job_complete: Mutex::new(None),
        }
    }

    pub(crate) async fn set_on_job_complete(
        &self,
        callback: Arc<dyn Fn(AgentJob) -> futures::future::BoxFuture<'static, ()> + Send + Sync>,
    ) {
        let mut lock = self.on_job_complete.lock().await;
        *lock = Some(callback);
    }

    // ── Public API ──────────────────────────────────────────────────────

    pub(crate) async fn launch(
        self: &Arc<Self>,
        profile_name: &str,
        user_intent: &str,
        working_directory: Option<&str>,
        conversation_id: Option<ConversationId>,
    ) -> Result<AgentJob, AppError> {
        let profile = self.profiles.get(profile_name).ok_or_else(|| {
            AppError::Validation(format!("Unknown agent profile: {profile_name}"))
        })?;

        if !profile.enabled {
            return Err(AppError::Validation(format!(
                "Agent profile is disabled: {profile_name}"
            )));
        }

        let permit = Arc::clone(&self.slots).try_acquire_owned().map_err(|_| {
            AppError::Validation(format!(
                "Maximum concurrent agents reached ({}). Wait for a running agent to finish or cancel one.",
                self.max_concurrent
            ))
        })?;

        let cmd_path = which::which(&profile.command).map_err(|_| {
            AppError::NotFound(format!(
                "Agent command not found: '{}'. Ensure it is installed and on PATH.",
                profile.command
            ))
        })?;
        validate_agent_command(profile_name, &cmd_path, &profile.args)?;

        let cwd = working_directory
            .map(String::from)
            .or_else(|| profile.working_directory.clone())
            .unwrap_or_else(|| {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .to_string_lossy()
                    .into_owned()
            });
        let cwd_path = PathBuf::from(&cwd).canonicalize().map_err(|_| {
            AppError::Validation(format!("Working directory does not exist: {cwd}"))
        })?;

        let cmd_args = build_command(profile, user_intent);

        let mut job = AgentJob::new(
            profile_name.to_owned(),
            format!("{} {}", cmd_path.display(), cmd_args.join(" ")),
            user_intent.to_owned(),
            cwd_path.to_string_lossy().into_owned(),
        );
        job.conversation_id = conversation_id;
        let mut job = self.repo.save(&job).await?;

        if let Err(error) = tokio::fs::create_dir_all(&self.output_dir).await {
            let message = format!("Failed to prepare agent output directory: {error}");
            self.record_launch_failure(&mut job, message.clone(), None)
                .await;
            return Err(AppError::Internal(message));
        }
        let output_path = self.output_dir.join(format!("{}.output", job.id));
        job.raw_output_path = Some(output_path.to_string_lossy().into_owned());

        let env = build_env(profile);

        let child = match Command::new(&cmd_path)
            .args(&cmd_args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .stdin(std::process::Stdio::null())
            .current_dir(&cwd_path)
            .envs(env)
            .spawn()
        {
            Ok(child) => child,
            Err(error) => {
                let message = format!("Failed to start subprocess: {error}");
                self.record_launch_failure(&mut job, message.clone(), None)
                    .await;
                return Err(AppError::Internal(message));
            }
        };

        let Some(pid) = child.id() else {
            let message = "Subprocess started but has no PID".to_string();
            self.record_launch_failure(&mut job, message.clone(), None)
                .await;
            return Err(AppError::Internal(message));
        };

        job.mark_running(i64::from(pid));
        let job = match self.repo.save(&job).await {
            Ok(job) => job,
            Err(error) => {
                let message = format!("Failed to persist running job state: {error}");
                self.record_launch_failure(&mut job, message.clone(), Some(pid))
                    .await;
                return Err(AppError::Internal(message));
            }
        };

        let manager = Arc::clone(self);
        let job_id = job.id;
        let output_format = profile.output_format.clone();
        let max_runtime = profile
            .max_runtime_seconds
            .unwrap_or(self.max_runtime_seconds);
        let output_path_clone = output_path.clone();

        tokio::spawn(async move {
            manager
                .watch_process(
                    job_id,
                    child,
                    output_path_clone,
                    &output_format,
                    max_runtime,
                    permit,
                )
                .await;
        });

        info!(
            job_id = %job.id,
            profile = profile_name,
            pid,
            cwd = %cwd_path.display(),
            "agent_job.launched"
        );
        Ok(job)
    }

    pub(crate) async fn cancel(&self, job_id: AgentJobId) -> Result<AgentJob, AppError> {
        let Some(mut job) = self.repo.get_by_id(job_id).await? else {
            return Err(AppError::NotFound(format!("Job {job_id} not found")));
        };

        if job.is_terminal() {
            return Ok(job);
        }

        if let Some(pid) = job.pid.and_then(|value| u32::try_from(value).ok()) {
            kill_process(pid).await;
        }

        job.mark_cancelled(Some("Cancelled by user".into()));
        let saved = self.repo.save(&job).await?;
        info!(job_id = %job_id, status = %saved.status, "agent_job.cancelled");
        Ok(saved)
    }

    // ── Private ─────────────────────────────────────────────────────────

    async fn record_launch_failure(
        &self,
        job: &mut AgentJob,
        error_message: String,
        pid_to_kill: Option<u32>,
    ) {
        if let Some(pid) = pid_to_kill {
            kill_process(pid).await;
        }

        job.mark_failed(error_message.clone(), None);
        if let Err(save_error) = self.repo.save(job).await {
            warn!(
                job_id = %job.id,
                launch_error = %error_message,
                error = %save_error,
                "agent_job.launch_failure_persist_failed"
            );
        }
    }

    async fn watch_process(
        &self,
        job_id: AgentJobId,
        mut child: tokio::process::Child,
        output_path: PathBuf,
        output_format: &str,
        max_runtime: u64,
        _permit: OwnedSemaphorePermit,
    ) {
        let capture_result = capture_process_output(&mut child, max_runtime, &output_path).await;

        let captured = match capture_result {
            Ok(captured) => captured,
            Err(e) => {
                error!(job_id = %job_id, error = %e, "agent_job.watcher_error");
                if let Ok(Some(mut job)) = self.repo.get_by_id(job_id).await
                    && !job.is_terminal()
                {
                    job.mark_failed(format!("Watcher error: {e}"), None);
                    drop(self.repo.save(&job).await);
                }
                return;
            }
        };

        self.finalize_job(
            job_id,
            captured.exit_code,
            &output_path,
            output_format,
            captured.runtime_error,
        )
        .await;
    }

    async fn finalize_job(
        &self,
        job_id: AgentJobId,
        exit_code: i32,
        output_path: &Path,
        output_format: &str,
        runtime_error: Option<String>,
    ) {
        let parsed = parse_output(output_path, output_format);

        if let Ok(Some(mut job)) = self.repo.get_by_id(job_id).await {
            if job.is_terminal() {
                return;
            }

            if exit_code == 0 && !parsed.is_error {
                job.mark_completed(exit_code, parsed.summary.clone());
            } else {
                let error = parsed
                    .error_detail
                    .clone()
                    .or(runtime_error)
                    .unwrap_or_else(|| format!("Process exited with code {exit_code}"));
                job.mark_failed(error, Some(exit_code));
                if let Some(ref summary) = parsed.summary {
                    job.result_summary = Some(summary.clone());
                }
            }

            job.cost_usd = parsed.cost_usd;
            job.agent_session_id = parsed.session_id.clone();
            if !parsed.files_changed.is_empty() {
                job.files_changed_json = serde_json::to_string(&parsed.files_changed).ok();
            }

            drop(self.repo.save(&job).await);

            info!(
                job_id = %job_id,
                status = %job.status,
                exit_code,
                cost = ?job.cost_usd,
                "agent_job.finished"
            );

            let callback = {
                let lock = self.on_job_complete.lock().await;
                lock.clone()
            };
            if let Some(cb) = callback {
                cb(job).await;
            }
        }
    }
}

impl std::fmt::Debug for AgentJobManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentJobManager")
            .field("output_dir", &self.output_dir)
            .field("max_concurrent", &self.max_concurrent)
            .field("profiles", &self.profiles.keys().collect::<Vec<_>>())
            .finish()
    }
}

fn build_command(profile: &AgentProfileConfig, user_intent: &str) -> Vec<String> {
    let mut args = profile.args.clone();
    if profile.command == "claude" {
        args.extend(["--".into(), user_intent.into()]);
    } else {
        args.push(user_intent.into());
    }
    args
}

fn build_env(profile: &AgentProfileConfig) -> HashMap<String, String> {
    let mut env = filter_safe_env_vars(std::env::vars(), &[]);
    if let Some(ref extra) = profile.env_vars {
        env.extend(filter_safe_env_vars(
            extra.iter().map(|(k, v)| (k.clone(), v.clone())),
            &[],
        ));
    }
    env
}

fn validate_agent_command(
    profile_name: &str,
    command: &Path,
    args: &[String],
) -> Result<(), AppError> {
    validate_subprocess_command_spec(
        &command.to_string_lossy(),
        args,
        AGENT_BLOCKED_COMMANDS.as_slice(),
        false,
    )
    .map_err(|error| {
        AppError::Validation(format!(
            "Agent profile '{profile_name}' failed security validation: {error}"
        ))
    })
}

fn parse_output(output_path: &std::path::Path, output_format: &str) -> AgentJobResult {
    if output_format == "ndjson" {
        parse_claude_ndjson(output_path)
    } else {
        parse_text_output(output_path)
    }
}

struct CapturedProcessOutput {
    exit_code: i32,
    runtime_error: Option<String>,
}

async fn capture_process_output(
    child: &mut tokio::process::Child,
    max_runtime: u64,
    output_path: &Path,
) -> Result<CapturedProcessOutput, std::io::Error> {
    let stdout_task = child.stdout.take().map(|stdout| {
        let output_path = output_path.to_path_buf();
        tokio::spawn(async move { capture_stdout(stdout, &output_path).await })
    });
    let stderr_task = child
        .stderr
        .take()
        .map(|stderr| tokio::spawn(async move { capture_stderr(stderr).await }));

    let mut timed_out = false;
    let exit_status = if let Ok(result) =
        tokio::time::timeout(tokio::time::Duration::from_secs(max_runtime), child.wait()).await
    {
        result?
    } else {
        timed_out = true;
        warn!(max_runtime, "agent_job.timeout");
        if let Some(pid) = child.id() {
            kill_process(pid).await;
        }
        child.wait().await?
    };

    join_stdout(stdout_task).await?;
    let stderr = join_stderr(stderr_task).await?;
    let stderr_summary = summarize_stderr(&stderr);
    let runtime_error = if timed_out {
        Some(match stderr_summary {
            Some(summary) => format!("Process timed out after {max_runtime}s. stderr: {summary}"),
            None => format!("Process timed out after {max_runtime}s"),
        })
    } else {
        stderr_summary
    };

    Ok(CapturedProcessOutput {
        exit_code: exit_status.code().unwrap_or(-1),
        runtime_error,
    })
}

async fn capture_stdout<R>(mut reader: R, output_path: &Path) -> std::io::Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let file = tokio::fs::File::create(output_path).await?;
    let mut writer = tokio::io::BufWriter::new(file);
    let mut buffer = [0_u8; 8192];
    let mut remaining = MAX_OUTPUT_BYTES;

    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            break;
        }

        let write_len = remaining.min(read);
        if write_len > 0 {
            writer.write_all(&buffer[..write_len]).await?;
            remaining -= write_len;
        }
    }

    writer.flush().await
}

async fn capture_stderr<R>(mut reader: R) -> std::io::Result<Vec<u8>>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut tail = Vec::new();
    let mut buffer = [0_u8; 4096];

    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            break;
        }

        push_tail_bytes(&mut tail, &buffer[..read], STDERR_SUMMARY_BYTES);
    }

    Ok(tail)
}

fn push_tail_bytes(tail: &mut Vec<u8>, chunk: &[u8], max_bytes: usize) {
    if max_bytes == 0 || chunk.is_empty() {
        return;
    }

    if chunk.len() >= max_bytes {
        tail.clear();
        tail.extend_from_slice(&chunk[chunk.len() - max_bytes..]);
        return;
    }

    let overflow = tail
        .len()
        .saturating_add(chunk.len())
        .saturating_sub(max_bytes);
    if overflow > 0 {
        tail.drain(..overflow);
    }

    tail.extend_from_slice(chunk);
}

async fn join_stdout(
    task: Option<tokio::task::JoinHandle<std::io::Result<()>>>,
) -> Result<(), std::io::Error> {
    let Some(task) = task else {
        return Ok(());
    };

    task.await
        .map_err(|e| std::io::Error::other(format!("Reader task failed: {e}")))?
}

async fn join_stderr(
    task: Option<tokio::task::JoinHandle<std::io::Result<Vec<u8>>>>,
) -> Result<Vec<u8>, std::io::Error> {
    let Some(task) = task else {
        return Ok(Vec::new());
    };

    task.await
        .map_err(|e| std::io::Error::other(format!("Reader task failed: {e}")))?
}

fn summarize_stderr(stderr: &[u8]) -> Option<String> {
    if stderr.is_empty() {
        return None;
    }

    let text = String::from_utf8_lossy(stderr);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    Some(crate::util::text::truncate_str(trimmed, 1_000).to_owned())
}

#[allow(unsafe_code)]
async fn kill_process(pid: u32) {
    use tokio::time::{Duration, sleep};

    #[cfg(unix)]
    {
        // SAFETY: libc::kill with a valid PID is safe; we obtained this PID from a
        // child process we spawned and tracked in the job registry.
        unsafe {
            libc::kill(i32::try_from(pid).unwrap_or(-1), libc::SIGTERM);
        }
        sleep(Duration::from_secs(KILL_GRACE_SECONDS)).await;
        // SAFETY: Same as above — SIGKILL as a fallback if the process did not exit.
        unsafe {
            libc::kill(i32::try_from(pid).unwrap_or(-1), libc::SIGKILL);
        }
    }

    #[cfg(not(unix))]
    {
        let _ = pid;
        warn!("Process killing not supported on this platform");
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::models::types::AgentJobStatus;
    use async_trait::async_trait;
    use std::sync::Arc;
    use tokio::sync::Mutex as TokioMutex;

    #[test]
    fn build_env_filters_loader_injection_keys() {
        let profile = AgentProfileConfig {
            name: "test".into(),
            command: "echo".into(),
            args: Vec::new(),
            output_format: "text".into(),
            enabled: true,
            max_runtime_seconds: None,
            working_directory: None,
            env_vars: Some(HashMap::from([
                ("SAFE_KEY".into(), "ok".into()),
                ("LD_PRELOAD".into(), "bad".into()),
            ])),
        };

        let env = build_env(&profile);

        assert_eq!(env.get("SAFE_KEY"), Some(&"ok".to_string()));
        assert!(!env.contains_key("LD_PRELOAD"));
    }

    #[test]
    fn build_env_keeps_pythonpath_for_compatibility() {
        let profile = AgentProfileConfig {
            name: "test".into(),
            command: "echo".into(),
            args: Vec::new(),
            output_format: "text".into(),
            enabled: true,
            max_runtime_seconds: None,
            working_directory: None,
            env_vars: Some(HashMap::from([("PYTHONPATH".into(), "custom".into())])),
        };

        let env = build_env(&profile);

        assert_eq!(env.get("PYTHONPATH"), Some(&"custom".to_string()));
    }

    #[cfg(unix)]
    #[test]
    fn validate_agent_command_allows_shell_wrapped_agents() {
        let bash = which::which("bash").unwrap();

        let result =
            validate_agent_command("test", &bash, &["-lc".into(), "claude --print".into()]);

        assert!(result.is_ok());
    }

    #[test]
    fn validate_agent_command_blocks_destructive_profiles() {
        let result = validate_subprocess_command_spec(
            "rm",
            &["-rf".into(), "/".into()],
            AGENT_BLOCKED_COMMANDS.as_slice(),
            false,
        );

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn cancel_marks_pending_job_cancelled() {
        let repo = Arc::new(TestAgentJobRepo::default());
        let job = AgentJob::new("profile".into(), "echo".into(), "task".into(), ".".into());
        repo.save(&job).await.unwrap();

        let manager = AgentJobManager::new(repo.clone(), HashMap::new(), PathBuf::from("."), 1, 60);
        let cancelled = manager.cancel(job.id).await.unwrap();

        assert_eq!(cancelled.status, AgentJobStatus::Cancelled);
        assert_eq!(
            repo.get_by_id(job.id).await.unwrap().unwrap().status,
            AgentJobStatus::Cancelled
        );
    }

    #[derive(Default)]
    struct TestAgentJobRepo {
        jobs: TokioMutex<HashMap<AgentJobId, AgentJob>>,
    }

    #[async_trait]
    impl AgentJobRepository for TestAgentJobRepo {
        async fn save(&self, job: &AgentJob) -> Result<AgentJob, AppError> {
            self.jobs.lock().await.insert(job.id, job.clone());
            Ok(job.clone())
        }

        async fn get_by_id(&self, id: AgentJobId) -> Result<Option<AgentJob>, AppError> {
            Ok(self.jobs.lock().await.get(&id).cloned())
        }
    }

    #[test]
    fn push_tail_bytes_keeps_latest_bytes_only() {
        let mut tail = Vec::new();

        push_tail_bytes(&mut tail, b"hello", 4);
        push_tail_bytes(&mut tail, b"world", 4);

        assert_eq!(tail, b"orld");
    }
}
