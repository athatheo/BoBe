//! Server / database / logging — infrastructure-level config domains.
//! All three are read once at boot (logging configures the subscriber;
//! server binds the listener; database opens the pool); ConfigManager's
//! hot-reload path doesn't touch these because changes need a restart.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct ServerConfig {
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) mdns_enabled: bool,
    pub(crate) cors_origins: Vec<String>,
    pub(crate) allowed_hosts: Vec<String>,
    #[serde(skip_serializing)]
    pub(crate) api_token: secrecy::SecretString,
    pub(crate) tls_cert_path: Option<String>,
    pub(crate) tls_key_path: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: crate::constants::DEFAULT_DAEMON_PORT,
            mdns_enabled: false,
            cors_origins: vec!["http://localhost:5175".into()],
            allowed_hosts: Vec::new(),
            api_token: secrecy::SecretString::default(),
            tls_cert_path: None,
            tls_key_path: None,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct DatabaseConfig {
    pub(crate) url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct LoggingConfig {
    pub(crate) level: String,
    pub(crate) json: bool,
    pub(crate) file: Option<String>,
    pub(crate) retention_days: u32,
    pub(crate) retention_count: u32,
    pub(crate) retention_total_bytes: u64,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "INFO".into(),
            json: false,
            file: None,
            retention_days: 14,
            retention_count: 14,
            retention_total_bytes: 100 * 1024 * 1024,
        }
    }
}
