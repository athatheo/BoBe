//! Install/uninstall Copilot CLI hook config in a worker's `.github/hooks/`.
//!
//! Copilot CLI loads every `*.json` in `<cwd>/.github/hooks/` and merges
//! them, so dropping a `bobe.json` co-exists with any user-owned hooks.
//! Events: `userPromptSubmitted` (5 s) + `notification` (10 s).

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

const SENTINEL: &str = "bobe.json";
const EVENTS: &[(&str, u64)] = &[("userPromptSubmitted", 5), ("notification", 10)];

pub(crate) fn hooks_dir(worker_dir: &Path) -> PathBuf {
    worker_dir.join(".github").join("hooks")
}

pub(crate) fn hooks_file(worker_dir: &Path) -> PathBuf {
    hooks_dir(worker_dir).join(SENTINEL)
}

/// Write `<worker_dir>/.github/hooks/bobe.json` so Copilot invokes
/// `hook_binary` on each event, with `BOBE_HOOK_SOCKET` injected via the
/// tmux session env (see `Mux::new_session`).
pub(crate) fn install(worker_dir: &Path, hook_binary: &Path) -> std::io::Result<PathBuf> {
    let dir = hooks_dir(worker_dir);
    std::fs::create_dir_all(&dir)?;

    let mut hooks_obj = serde_json::Map::new();
    for (event, timeout) in EVENTS {
        let entry = json!({
            "type": "command",
            "bash": hook_binary.to_string_lossy(),
            "timeoutSec": *timeout,
        });
        hooks_obj.insert((*event).to_string(), Value::Array(vec![entry]));
    }
    let doc = json!({
        "version": 1,
        "hooks": Value::Object(hooks_obj),
    });

    let path = hooks_file(worker_dir);
    let body = serde_json::to_vec_pretty(&doc)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    atomic_write(&path, &body)?;
    Ok(path)
}

#[allow(dead_code, reason = "Phase 1: test-only; lifecycle uninstall lands in Phase 2")]
pub(crate) fn uninstall(worker_dir: &Path) -> std::io::Result<()> {
    let path = hooks_file(worker_dir);
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "no parent"))?;
    let tmp = parent.join(format!(
        ".{}.tmp",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("hook")
    ));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "tests panic on precondition failures")]
mod tests {
    use super::*;

    #[test]
    fn install_writes_sentinel() {
        let dir = tempdir();
        let bin = std::path::Path::new("/usr/local/bin/bobe-copilot-hook");
        let path = install(&dir, bin).unwrap();
        assert!(path.exists());

        let doc: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(doc["version"], 1);
        for (event, _) in EVENTS {
            let arr = doc["hooks"][event].as_array().unwrap();
            assert_eq!(arr.len(), 1);
            assert_eq!(arr[0]["type"], "command");
            assert_eq!(arr[0]["bash"].as_str().unwrap(), bin.to_string_lossy());
        }
    }

    #[test]
    fn install_is_idempotent() {
        let dir = tempdir();
        let bin = std::path::Path::new("/tmp/hook");
        install(&dir, bin).unwrap();
        install(&dir, bin).unwrap();
        let doc: Value = serde_json::from_slice(&std::fs::read(hooks_file(&dir)).unwrap()).unwrap();
        for (event, _) in EVENTS {
            assert_eq!(doc["hooks"][event].as_array().unwrap().len(), 1);
        }
    }

    #[test]
    fn uninstall_removes_sentinel() {
        let dir = tempdir();
        install(&dir, std::path::Path::new("/tmp/hook")).unwrap();
        uninstall(&dir).unwrap();
        assert!(!hooks_file(&dir).exists());
    }

    fn tempdir() -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "bobe-hook-install-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }
}
