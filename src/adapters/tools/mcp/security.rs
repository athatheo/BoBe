use crate::error::AppError;

/// Default blocked commands (used when no config override).
pub const DEFAULT_BLOCKED_COMMANDS: &[&str] = &[
    "rm",
    "rmdir",
    "dd",
    "mkfs",
    "fdisk",
    "format",
    "del",
    "sh",
    "bash",
    "zsh",
    "fish",
    "cmd",
    "powershell",
    "pwsh",
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
];

/// Default dangerous env keys (used when no config override).
pub const DEFAULT_DANGEROUS_ENV_KEYS: &[&str] = &[
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "DYLD_INSERT_LIBRARIES",
    "DYLD_LIBRARY_PATH",
];

/// Validate that a command is not in the blocklist.
pub fn validate_mcp_command(command: &str, blocked: &[String]) -> Result<(), AppError> {
    // Extract basename from full path
    let basename = std::path::Path::new(command)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(command);

    // Check config-provided blocklist first, fall back to defaults
    let is_blocked = if blocked.is_empty() {
        DEFAULT_BLOCKED_COMMANDS.contains(&basename)
    } else {
        blocked.iter().any(|b| b == basename)
    };

    if is_blocked {
        return Err(AppError::Mcp(format!(
            "Command '{}' is blocked for security reasons",
            basename
        )));
    }

    Ok(())
}

/// Validate that environment variables don't contain dangerous keys.
pub fn validate_mcp_env(
    env: &std::collections::HashMap<String, String>,
    dangerous_keys: &[String],
) -> Result<(), AppError> {
    let check_keys: Vec<&str> = if dangerous_keys.is_empty() {
        DEFAULT_DANGEROUS_ENV_KEYS.to_vec()
    } else {
        dangerous_keys.iter().map(|s| s.as_str()).collect()
    };

    let forbidden: Vec<&str> = env
        .keys()
        .filter(|k| check_keys.contains(&k.as_str()))
        .map(|k| k.as_str())
        .collect();

    if !forbidden.is_empty() {
        return Err(AppError::Mcp(format!(
            "Dangerous environment variables not allowed: {}",
            forbidden.join(", ")
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_validate_command_allows_safe() {
        assert!(validate_mcp_command("npx", &[]).is_ok());
        assert!(validate_mcp_command("uvx", &[]).is_ok());
        assert!(validate_mcp_command("docker", &[]).is_ok());
        assert!(validate_mcp_command("node", &[]).is_ok());
    }

    #[test]
    fn test_validate_command_blocks_dangerous() {
        assert!(validate_mcp_command("rm", &[]).is_err());
        assert!(validate_mcp_command("/bin/rm", &[]).is_err());
        assert!(validate_mcp_command("sudo", &[]).is_err());
        assert!(validate_mcp_command("bash", &[]).is_err());
    }

    #[test]
    fn test_validate_command_with_custom_blocklist() {
        let custom = vec!["npm".into(), "yarn".into()];
        assert!(validate_mcp_command("npm", &custom).is_err());
        assert!(validate_mcp_command("bash", &custom).is_ok()); // not in custom list
    }

    #[test]
    fn test_validate_env_allows_safe() {
        let env: HashMap<String, String> =
            [("PATH".into(), "/usr/bin".into())].into();
        assert!(validate_mcp_env(&env, &[]).is_ok());
    }

    #[test]
    fn test_validate_env_blocks_dangerous() {
        let env: HashMap<String, String> =
            [("LD_PRELOAD".into(), "evil.so".into())].into();
        assert!(validate_mcp_env(&env, &[]).is_err());
    }
}
