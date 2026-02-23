use crate::error::AppError;

/// Blocked commands that MCP servers are not allowed to use.
pub const BLOCKED_COMMANDS: &[&str] = &[
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

/// Environment variable keys that are dangerous to allow injection of.
pub const DANGEROUS_ENV_KEYS: &[&str] = &[
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "DYLD_INSERT_LIBRARIES",
    "DYLD_LIBRARY_PATH",
    "DYLD_FRAMEWORK_PATH",
    "PYTHONPATH",
    "RUBYLIB",
    "NODE_PATH",
    "CLASSPATH",
    "PERL5LIB",
    "HOME",
    "USER",
    "LOGNAME",
    "SHELL",
];

/// Validate that a command is not in the blocklist.
pub fn validate_mcp_command(command: &str) -> Result<(), AppError> {
    // Extract basename from full path
    let basename = std::path::Path::new(command)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(command);

    if BLOCKED_COMMANDS.contains(&basename) {
        return Err(AppError::Mcp(format!(
            "Command '{}' is blocked for security reasons",
            basename
        )));
    }

    Ok(())
}

/// Validate that environment variables don't contain dangerous keys.
pub fn validate_mcp_env(env: &std::collections::HashMap<String, String>) -> Result<(), AppError> {
    let forbidden: Vec<&str> = env
        .keys()
        .filter(|k| DANGEROUS_ENV_KEYS.contains(&k.as_str()))
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
        assert!(validate_mcp_command("npx").is_ok());
        assert!(validate_mcp_command("uvx").is_ok());
        assert!(validate_mcp_command("docker").is_ok());
        assert!(validate_mcp_command("node").is_ok());
    }

    #[test]
    fn test_validate_command_blocks_dangerous() {
        assert!(validate_mcp_command("rm").is_err());
        assert!(validate_mcp_command("/bin/rm").is_err());
        assert!(validate_mcp_command("sudo").is_err());
        assert!(validate_mcp_command("bash").is_err());
    }

    #[test]
    fn test_validate_env_allows_safe() {
        let env: HashMap<String, String> =
            [("PATH".into(), "/usr/bin".into())].into();
        assert!(validate_mcp_env(&env).is_ok());
    }

    #[test]
    fn test_validate_env_blocks_dangerous() {
        let env: HashMap<String, String> =
            [("LD_PRELOAD".into(), "evil.so".into())].into();
        assert!(validate_mcp_env(&env).is_err());
    }
}
