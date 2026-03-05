use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{debug, error, info};

use super::config::McpParsedServer;
use crate::error::AppError;

#[derive(serde::Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    id: u64,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
}

#[derive(serde::Deserialize)]
struct JsonRpcResponse {
    #[serde(rename = "id")]
    _id: Option<u64>,
    result: Option<Value>,
    error: Option<JsonRpcError>,
}

#[derive(serde::Deserialize)]
struct JsonRpcError {
    #[serde(rename = "code")]
    _code: i64,
    message: String,
}

pub(crate) struct McpClient {
    config: McpParsedServer,
    process: Mutex<Option<McpProcess>>,
    request_id: AtomicU64,
    connected: std::sync::atomic::AtomicBool,
    last_error: Mutex<Option<String>>,
}

struct McpProcess {
    child: Child,
    stdin: std::process::ChildStdin,
    reader: BufReader<std::process::ChildStdout>,
}

impl McpClient {
    pub(crate) fn new(config: McpParsedServer) -> Self {
        Self {
            config,
            process: Mutex::new(None),
            request_id: AtomicU64::new(1),
            connected: std::sync::atomic::AtomicBool::new(false),
            last_error: Mutex::new(None),
        }
    }

    pub(crate) fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Acquire)
    }

    pub(crate) async fn last_error(&self) -> Option<String> {
        self.last_error.lock().await.clone()
    }

    pub(crate) fn timeout_seconds(&self) -> f64 {
        self.config.timeout_seconds
    }

    pub(crate) async fn connect(&self) -> Result<(), AppError> {
        info!(server = %self.config.name, command = %self.config.command, "Connecting to MCP server");

        let mut cmd = Command::new(&self.config.command);
        cmd.args(&self.config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        for (k, v) in &self.config.env {
            cmd.env(k, v);
        }

        let mut child = cmd.spawn().map_err(|e| {
            let msg = format!(
                "Failed to spawn MCP server '{}' ({}): {e}",
                self.config.name, self.config.command
            );
            error!("{}", msg);
            AppError::Mcp(msg)
        })?;

        let stdin = child.stdin.take().ok_or_else(|| {
            AppError::Mcp(format!("No stdin for MCP server '{}'", self.config.name))
        })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            AppError::Mcp(format!("No stdout for MCP server '{}'", self.config.name))
        })?;
        if let Err(e) = set_nonblocking_stdout(&stdout) {
            drop(child.kill());
            drop(child.wait());
            return Err(e);
        }

        let reader = BufReader::new(stdout);

        let mut proc = self.process.lock().await;
        *proc = Some(McpProcess {
            child,
            stdin,
            reader,
        });
        drop(proc);

        let init_result = self
            .send_request(
                "initialize",
                Some(serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": {
                        "name": "bobe",
                        "version": "0.1.0"
                    }
                })),
            )
            .await;

        match init_result {
            Ok(_) => {
                drop(
                    self.send_notification("notifications/initialized", None)
                        .await,
                );
                self.connected.store(true, Ordering::Release);
                info!(server = %self.config.name, "MCP server connected");
                Ok(())
            }
            Err(e) => {
                self.disconnect().await;
                *self.last_error.lock().await = Some(e.to_string());
                Err(e)
            }
        }
    }

    pub(crate) async fn disconnect(&self) {
        let mut proc = self.process.lock().await;
        if let Some(mut p) = proc.take() {
            drop(p.child.kill());
            drop(
                tokio::task::spawn_blocking(move || {
                    for _ in 0..30 {
                        if let Ok(Some(_)) = p.child.try_wait() {
                            return;
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                })
                .await,
            );
        }
        self.connected.store(false, Ordering::Release);
        debug!(server = %self.config.name, "MCP server disconnected");
    }

    pub(crate) async fn list_tools(&self) -> Result<Vec<McpToolInfo>, AppError> {
        let result = self.send_request("tools/list", None).await?;

        let tools = result
            .get("tools")
            .and_then(|t| t.as_array())
            .cloned()
            .unwrap_or_default();

        let mut infos = Vec::new();
        for tool in tools {
            let name = tool
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_owned();
            let description = tool
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("")
                .to_owned();
            let input_schema = tool
                .get("inputSchema")
                .cloned()
                .unwrap_or(serde_json::json!({"type": "object", "properties": {}}));

            infos.push(McpToolInfo {
                name,
                description,
                input_schema,
            });
        }
        Ok(infos)
    }

    pub(crate) async fn call_tool(
        &self,
        name: &str,
        arguments: HashMap<String, Value>,
    ) -> Result<(bool, String), AppError> {
        let result = self
            .send_request(
                "tools/call",
                Some(serde_json::json!({
                    "name": name,
                    "arguments": arguments,
                })),
            )
            .await?;

        let is_error = result
            .get("isError")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let content_parts = result
            .get("content")
            .and_then(|c| c.as_array())
            .cloned()
            .unwrap_or_default();

        let mut content = String::new();
        for part in &content_parts {
            if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                if !content.is_empty() {
                    content.push('\n');
                }
                content.push_str(text);
            }
        }

        Ok((!is_error, content))
    }

    async fn send_request(&self, method: &str, params: Option<Value>) -> Result<Value, AppError> {
        let id = self.request_id.fetch_add(1, Ordering::Relaxed);

        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id,
            method: method.to_owned(),
            params,
        };

        let request_str = serde_json::to_string(&request)
            .map_err(|e| AppError::Mcp(format!("Failed to serialize request: {e}")))?;

        let mut proc = self.process.lock().await;
        let process = proc
            .as_mut()
            .ok_or_else(|| AppError::Mcp("MCP server not connected".into()))?;

        writeln!(process.stdin, "{request_str}")
            .map_err(|e| AppError::Mcp(format!("Failed to write to MCP server: {e}")))?;
        process
            .stdin
            .flush()
            .map_err(|e| AppError::Mcp(format!("Failed to flush MCP server stdin: {e}")))?;

        let timeout = Duration::from_secs_f64(self.config.timeout_seconds.max(1.0));
        let server_name = self.config.name.clone();
        let method_name = method.to_owned();

        let reader = &mut process.reader;
        let child = &mut process.child;

        let mut line = String::new();
        let deadline = Instant::now() + timeout;
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    let status = child.try_wait().ok().flatten();
                    let details = status
                        .map(|s| format!(" (exit status: {s})"))
                        .unwrap_or_default();
                    return Err(AppError::Mcp(format!(
                        "MCP server '{server_name}' closed stdout before responding{details}"
                    )));
                }
                Ok(_) => break,
                Err(e) if e.kind() == ErrorKind::WouldBlock => {
                    if let Ok(Some(status)) = child.try_wait() {
                        return Err(AppError::Mcp(format!(
                            "MCP server '{server_name}' exited before responding (status: {status})"
                        )));
                    }
                    if Instant::now() >= deadline {
                        return Err(AppError::Mcp(format!(
                            "MCP server '{server_name}' did not respond to '{method_name}' within {:.1}s",
                            timeout.as_secs_f64()
                        )));
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Err(e) => {
                    return Err(AppError::Mcp(format!(
                        "Failed to read from MCP server '{server_name}': {e}"
                    )));
                }
            }
        }

        let response: JsonRpcResponse = serde_json::from_str(line.trim())
            .map_err(|e| AppError::Mcp(format!("Invalid JSON-RPC response: {e}")))?;

        if let Some(err) = response.error {
            return Err(AppError::Mcp(format!("MCP server error: {}", err.message)));
        }

        response
            .result
            .ok_or_else(|| AppError::Mcp("Empty response from MCP server".into()))
    }

    async fn send_notification(&self, method: &str, params: Option<Value>) -> Result<(), AppError> {
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params.unwrap_or(Value::Null),
        });

        let notification_str = serde_json::to_string(&notification)
            .map_err(|e| AppError::Mcp(format!("Failed to serialize notification: {e}")))?;

        let mut proc = self.process.lock().await;
        let process = proc
            .as_mut()
            .ok_or_else(|| AppError::Mcp("MCP server not connected".into()))?;

        writeln!(process.stdin, "{notification_str}")
            .map_err(|e| AppError::Mcp(format!("Failed to write notification: {e}")))?;
        process
            .stdin
            .flush()
            .map_err(|e| AppError::Mcp(format!("Failed to flush notification: {e}")))?;

        Ok(())
    }
}

#[cfg(unix)]
fn set_nonblocking_stdout(stdout: &std::process::ChildStdout) -> Result<(), AppError> {
    let mut flags = rustix::fs::fcntl_getfl(stdout).map_err(|e| {
        AppError::Mcp(format!(
            "Failed to read MCP stdout flags for nonblocking mode: {e}"
        ))
    })?;
    flags.insert(rustix::fs::OFlags::NONBLOCK);
    rustix::fs::fcntl_setfl(stdout, flags)
        .map_err(|e| AppError::Mcp(format!("Failed to set MCP stdout nonblocking mode: {e}")))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_nonblocking_stdout(_stdout: &std::process::ChildStdout) -> Result<(), AppError> {
    Ok(())
}

impl Drop for McpClient {
    fn drop(&mut self) {
        if let Ok(mut proc) = self.process.try_lock()
            && let Some(mut p) = proc.take()
        {
            drop(p.child.kill());
            std::thread::spawn(move || {
                for _ in 0..30 {
                    if let Ok(Some(_)) = p.child.try_wait() {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            });
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct McpToolInfo {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) input_schema: Value,
}
