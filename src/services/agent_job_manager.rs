//! AgentJobManager — manages coding agent subprocess lifecycle.
//!
//! Launches coding agents (Claude Code, Aider, OpenCode) as headless subprocesses.
//! Handles spawning, monitoring, output collection, and cleanup.
//!
//! Concurrency: singleton, protected by async lock, max_concurrent limit.
//! Cancellation: cancel() sends SIGTERM then SIGKILL.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::io::AsyncBufReadExt;
use tokio::process::Command;
use tokio::sync::Mutex;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::models::agent_job::AgentJob;
use crate::models::types::AgentJobStatus;
use crate::error::AppError;
use crate::db::AgentJobRepository;

use super::agent_output_parsers::{AgentJobResult, parse_claude_ndjson, parse_text_output};

/// Safety: max output file size (10 MB).
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

/// Seconds to wait after SIGTERM before SIGKILL.
const KILL_GRACE_SECONDS: u64 = 5;

/// Dangerous env vars that must NOT be passed to subprocesses.
const BLOCKED_ENV_KEYS: &[&str] = &[
    "LD_PRELOAD",
    "DYLD_INSERT_LIBRARIES",
    "LD_LIBRARY_PATH",
    "PYTHONPATH",
];

/// Configuration for a single coding agent CLI profile.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentProfileConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    #[serde(default = "default_output_format")]
    pub output_format: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub max_runtime_seconds: Option<u64>,
    pub working_directory: Option<String>,
    pub env_vars: Option<HashMap<String, String>>,
}

fn default_output_format() -> String {
    "text".into()
}

fn default_true() -> bool {
    true
}

/// In-memory state for a running subprocess.
#[allow(dead_code)]
struct RunningJob {
    watcher_handle: tokio::task::JoinHandle<()>,
    pid: u32,
}

pub struct AgentJobManager {
    repo: Arc<dyn AgentJobRepository>,
    profiles: HashMap<String, AgentProfileConfig>,
    output_dir: PathBuf,
    max_concurrent: usize,
    max_runtime_seconds: u64,
    running_jobs: Mutex<HashMap<Uuid, RunningJob>>,
    on_job_complete: Mutex<
        Option<Arc<dyn Fn(AgentJob) -> futures::future::BoxFuture<'static, ()> + Send + Sync>>,
    >,
}

impl AgentJobManager {
    pub fn new(
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
            running_jobs: Mutex::new(HashMap::new()),
            on_job_complete: Mutex::new(None),
        }
    }

    /// Set callback fired when a job reaches a terminal state.
    pub async fn set_on_job_complete(
        &self,
        callback: Arc<dyn Fn(AgentJob) -> futures::future::BoxFuture<'static, ()> + Send + Sync>,
    ) {
        let mut lock = self.on_job_complete.lock().await;
        *lock = Some(callback);
    }

    /// Get list of enabled agent profiles for discovery.
    #[allow(dead_code)]
    pub fn get_available_profiles(&self) -> Vec<&AgentProfileConfig> {
        self.profiles.values().filter(|p| p.enabled).collect()
    }

    // ── Public API ──────────────────────────────────────────────────────

    /// Launch a coding agent subprocess.
    pub async fn launch(
        self: &Arc<Self>,
        profile_name: &str,
        user_intent: &str,
        working_directory: Option<&str>,
        conversation_id: Option<Uuid>,
    ) -> Result<AgentJob, AppError> {
        let profile = self.profiles.get(profile_name).ok_or_else(|| {
            AppError::Validation(format!("Unknown agent profile: {profile_name}"))
        })?;

        if !profile.enabled {
            return Err(AppError::Validation(format!(
                "Agent profile is disabled: {profile_name}"
            )));
        }

        // Check concurrent limit
        {
            let running = self.running_jobs.lock().await;
            if running.len() >= self.max_concurrent {
                return Err(AppError::Validation(format!(
                    "Maximum concurrent agents reached ({}). Wait for a running agent to finish or cancel one.",
                    self.max_concurrent
                )));
            }
        }

        // Verify command exists
        let cmd_path = which::which(&profile.command).map_err(|_| {
            AppError::NotFound(format!(
                "Agent command not found: '{}'. Ensure it is installed and on PATH.",
                profile.command
            ))
        })?;

        // Resolve working directory
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

        // Build command
        let cmd_args = build_command(profile, user_intent);

        // Create job record
        let mut job = AgentJob::new(
            profile_name.to_owned(),
            format!("{} {}", cmd_path.display(), cmd_args.join(" ")),
            user_intent.to_owned(),
            cwd_path.to_string_lossy().into_owned(),
        );
        job.conversation_id = conversation_id;
        let mut job = self.repo.save(&job).await?;

        // Prepare output file
        tokio::fs::create_dir_all(&self.output_dir).await?;
        let output_path = self.output_dir.join(format!("{}.output", job.id));
        job.raw_output_path = Some(output_path.to_string_lossy().into_owned());

        // Build env
        let env = build_env(profile);

        // Start subprocess
        let child = Command::new(&profile.command)
            .args(&cmd_args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .stdin(std::process::Stdio::null())
            .current_dir(&cwd_path)
            .envs(env)
            .spawn()
            .map_err(|e| AppError::Internal(format!("Failed to start subprocess: {e}")))?;

        let pid = child
            .id()
            .ok_or_else(|| AppError::Internal("Subprocess started but has no PID".into()))?;

        job.mark_running(pid as i64);
        let job = self.repo.save(&job).await?;

        // Spawn watcher
        let manager = Arc::clone(self);
        let job_id = job.id;
        let output_format = profile.output_format.clone();
        let max_runtime = profile
            .max_runtime_seconds
            .unwrap_or(self.max_runtime_seconds);
        let output_path_clone = output_path.clone();

        let watcher_handle = tokio::spawn(async move {
            manager
                .watch_process(
                    job_id,
                    child,
                    output_path_clone,
                    &output_format,
                    max_runtime,
                )
                .await;
        });

        {
            let mut running = self.running_jobs.lock().await;
            running.insert(
                job_id,
                RunningJob {
                    watcher_handle,
                    pid,
                },
            );
        }

        info!(
            job_id = %job.id,
            profile = profile_name,
            pid,
            cwd = %cwd_path.display(),
            "agent_job.launched"
        );
        Ok(job)
    }

    /// Check the current status of a job.
    #[allow(dead_code)]
    pub async fn check(&self, job_id: Uuid) -> Result<Option<AgentJob>, AppError> {
        self.repo.get_by_id(job_id).await
    }

    /// Cancel a running job.
    #[allow(dead_code)]
    pub async fn cancel(&self, job_id: Uuid) -> Result<bool, AppError> {
        let running = {
            let mut running = self.running_jobs.lock().await;
            running.remove(&job_id)
        };

        if let Some(rj) = running {
            kill_process(rj.pid).await;
            rj.watcher_handle.abort();
        }

        // Mark as cancelled in DB
        if let Some(mut job) = self.repo.get_by_id(job_id).await?
            && !job.is_terminal()
        {
            job.mark_cancelled(Some("user request".to_owned()));
            self.repo.save(&job).await?;
            info!(job_id = %job_id, "agent_job.cancelled");
            return Ok(true);
        }
        Ok(false)
    }

    /// Resume a completed job with additional instructions.
    ///
    /// Uses the agent's session_id to resume the same session (Claude Code).
    /// For agents without session resume, starts a fresh run with context.
    #[allow(dead_code)]
    pub async fn continue_job(
        self: &Arc<Self>,
        job_id: Uuid,
        continuation_prompt: &str,
    ) -> Result<Option<AgentJob>, AppError> {
        let original = self.repo.get_by_id(job_id).await?;
        let original = match original {
            Some(j) if j.is_terminal() => j,
            _ => return Ok(None),
        };

        let profile = match self.profiles.get(&original.profile_name) {
            Some(p) if p.enabled => p.clone(),
            _ => return Ok(None),
        };

        // Enforce max continuations
        if original.continuation_count >= 3 {
            return Ok(None);
        }

        // Build continuation command
        let cmd_args = if let (true, Some(session_id)) = (
            profile.command == "claude",
            original.agent_session_id.as_ref(),
        ) {
            // Claude Code: resume the existing session
            let mut args = profile.args.clone();
            args.extend([
                "--resume".into(),
                session_id.clone(),
                "--".into(),
                continuation_prompt.to_owned(),
            ]);
            args
        } else {
            build_command(&profile, continuation_prompt)
        };

        // Verify command exists
        let cmd_path = which::which(&profile.command).map_err(|_| {
            AppError::NotFound(format!(
                "Agent command not found: '{}'. Ensure it is installed and on PATH.",
                profile.command
            ))
        })?;

        // Create new job linked to original
        let mut job = AgentJob::new(
            original.profile_name.clone(),
            format!("{} {}", cmd_path.display(), cmd_args.join(" ")),
            continuation_prompt.to_owned(),
            original.working_directory.clone(),
        );
        job.conversation_id = original.conversation_id;
        job.continuation_count = original.continuation_count + 1;
        let mut job = self.repo.save(&job).await?;

        // Prepare output file
        tokio::fs::create_dir_all(&self.output_dir).await?;
        let output_path = self.output_dir.join(format!("{}.output", job.id));
        job.raw_output_path = Some(output_path.to_string_lossy().into_owned());

        // Build env and start subprocess
        let env = build_env(&profile);
        let child = Command::new(&profile.command)
            .args(&cmd_args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .stdin(std::process::Stdio::null())
            .current_dir(&original.working_directory)
            .envs(env)
            .spawn()
            .map_err(|e| {
                AppError::Internal(format!("Failed to start continuation subprocess: {e}"))
            })?;

        let pid = child.id().ok_or_else(|| {
            AppError::Internal("Continuation subprocess started but has no PID".into())
        })?;

        job.mark_running(pid as i64);
        let job = self.repo.save(&job).await?;

        // Spawn watcher
        let manager = Arc::clone(self);
        let new_job_id = job.id;
        let output_format = profile.output_format.clone();
        let max_runtime = profile
            .max_runtime_seconds
            .unwrap_or(self.max_runtime_seconds);
        let output_path_clone = output_path.clone();

        let watcher_handle = tokio::spawn(async move {
            manager
                .watch_process(
                    new_job_id,
                    child,
                    output_path_clone,
                    &output_format,
                    max_runtime,
                )
                .await;
        });

        {
            let mut running = self.running_jobs.lock().await;
            running.insert(
                new_job_id,
                RunningJob {
                    watcher_handle,
                    pid,
                },
            );
        }

        info!(
            job_id = %new_job_id,
            original_job_id = %job_id,
            continuation = job.continuation_count,
            "agent_job.continued"
        );
        Ok(Some(job))
    }
    #[allow(dead_code)]
    pub async fn poll_completed_unreported(&self) -> Result<Vec<AgentJob>, AppError> {
        self.repo.find_unreported_terminal().await
    }

    /// Kill all running agents on server shutdown.
    #[allow(dead_code)]
    pub async fn cleanup_on_shutdown(&self) {
        let jobs_to_kill: Vec<(Uuid, u32)> = {
            let mut running = self.running_jobs.lock().await;
            let items: Vec<(Uuid, u32)> = running.iter().map(|(id, rj)| (*id, rj.pid)).collect();
            running.clear();
            items
        };

        for (job_id, pid) in jobs_to_kill {
            info!(job_id = %job_id, "agent_job.shutdown_kill");
            kill_process(pid).await;
            if let Ok(Some(mut job)) = self.repo.get_by_id(job_id).await
                && !job.is_terminal()
            {
                job.mark_cancelled(Some("server shutdown".to_owned()));
                let _ = self.repo.save(&job).await;
            }
        }
    }

    /// Mark orphaned running/pending jobs as failed on startup.
    #[allow(dead_code)]
    pub async fn recover_orphaned_jobs(&self) -> Result<u32, AppError> {
        let mut count = 0u32;

        for status in [AgentJobStatus::Running, AgentJobStatus::Pending] {
            let orphaned = self.repo.find_by_status(status).await?;
            for mut job in orphaned {
                let reason = if status == AgentJobStatus::Pending {
                    "Server restart - job never started"
                } else {
                    "Server restart - process no longer running"
                };
                job.mark_failed(reason.to_owned(), None);
                self.repo.save(&job).await?;
                count += 1;
                info!(
                    job_id = %job.id,
                    previous_status = status.as_str(),
                    "agent_job.orphan_recovered"
                );
            }
        }
        Ok(count)
    }

    // ── Private ─────────────────────────────────────────────────────────

    async fn watch_process(
        &self,
        job_id: Uuid,
        mut child: tokio::process::Child,
        output_path: PathBuf,
        output_format: &str,
        max_runtime: u64,
    ) {
        let mut bytes_written: usize = 0;

        // Read stdout into file
        let stdout = child.stdout.take();
        let write_result = async {
            let file = tokio::fs::File::create(&output_path).await?;
            let mut writer = tokio::io::BufWriter::new(file);

            if let Some(stdout) = stdout {
                let reader = tokio::io::BufReader::new(stdout);
                let mut lines = reader.lines();

                let timeout = tokio::time::Duration::from_secs(max_runtime);
                let deadline = tokio::time::Instant::now() + timeout;

                loop {
                    let line_result = tokio::time::timeout_at(deadline, lines.next_line()).await;

                    match line_result {
                        Ok(Ok(Some(line))) => {
                            if bytes_written < MAX_OUTPUT_BYTES {
                                use tokio::io::AsyncWriteExt;
                                let line_bytes = format!("{line}\n");
                                writer.write_all(line_bytes.as_bytes()).await?;
                                bytes_written += line_bytes.len();
                            }
                        }
                        Ok(Ok(None)) => break, // EOF
                        Ok(Err(e)) => {
                            warn!(job_id = %job_id, error = %e, "agent_job.read_error");
                            break;
                        }
                        Err(_) => {
                            warn!(job_id = %job_id, max_runtime, "agent_job.timeout");
                            if let Some(pid) = child.id() {
                                kill_process(pid).await;
                            }
                            break;
                        }
                    }
                }

                use tokio::io::AsyncWriteExt;
                writer.flush().await?;
            }
            Ok::<_, std::io::Error>(())
        }
        .await;

        if let Err(e) = write_result {
            error!(job_id = %job_id, error = %e, "agent_job.watcher_error");
            if let Ok(Some(mut job)) = self.repo.get_by_id(job_id).await
                && !job.is_terminal()
            {
                job.mark_failed(format!("Watcher error: {e}"), None);
                let _ = self.repo.save(&job).await;
            }
            return;
        }

        // Wait for process exit
        let exit_status = child.wait().await;
        let exit_code = exit_status.ok().and_then(|s| s.code()).unwrap_or(-1);

        // Remove from running jobs
        {
            let mut running = self.running_jobs.lock().await;
            running.remove(&job_id);
        }

        // Parse output and update job
        let parsed = parse_output(&output_path, output_format);

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

            let _ = self.repo.save(&job).await;

            info!(
                job_id = %job_id,
                status = %job.status,
                exit_code,
                cost = ?job.cost_usd,
                "agent_job.finished"
            );

            // Fire completion callback
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
    let blocked: std::collections::HashSet<&str> = BLOCKED_ENV_KEYS.iter().copied().collect();
    let mut env: HashMap<String, String> = std::env::vars()
        .filter(|(k, _)| !blocked.contains(k.as_str()))
        .collect();
    if let Some(ref extra) = profile.env_vars {
        env.extend(extra.clone());
    }
    env
}

fn parse_output(output_path: &std::path::Path, output_format: &str) -> AgentJobResult {
    if output_format == "ndjson" {
        parse_claude_ndjson(output_path)
    } else {
        parse_text_output(output_path)
    }
}

#[allow(unsafe_code)]
async fn kill_process(pid: u32) {
    use tokio::time::{Duration, sleep};

    // Send SIGTERM
    #[cfg(unix)]
    {
        // SAFETY: libc::kill with a valid PID is safe; we obtained this PID from a
        // child process we spawned and tracked in the job registry.
        unsafe {
            libc::kill(pid as i32, libc::SIGTERM);
        }
        sleep(Duration::from_secs(KILL_GRACE_SECONDS)).await;
        // SAFETY: Same as above — SIGKILL as a fallback if the process did not exit.
        unsafe {
            libc::kill(pid as i32, libc::SIGKILL);
        }
    }

    #[cfg(not(unix))]
    {
        let _ = pid;
        warn!("Process killing not supported on this platform");
    }
}
