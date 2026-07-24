use secrecy::SecretString;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct BodyConfig {
    pub(crate) enabled: bool,
    pub(crate) host: String,
    pub(crate) advertised_hostname: String,
    pub(crate) enrolled_device_id: String,
    pub(crate) port: u16,
    pub(crate) adapter_port: u16,
    #[serde(skip_serializing)]
    pub(crate) adapter_token: SecretString,
    pub(crate) tls_cert_path: Option<String>,
    pub(crate) tls_key_path: Option<String>,
    pub(crate) client_ca_path: Option<String>,
    pub(crate) enrolled_client_cert_path: Option<String>,
    pub(crate) trust_key_id: String,
    pub(crate) controller_epoch: u64,
    pub(crate) mdns_enabled: bool,
}

impl Default for BodyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            host: "127.0.0.1".into(),
            advertised_hostname: "bobe-body-hub.local".into(),
            enrolled_device_id: String::new(),
            port: 8_767,
            adapter_port: 8_768,
            adapter_token: SecretString::default(),
            tls_cert_path: None,
            tls_key_path: None,
            client_ca_path: None,
            enrolled_client_cert_path: None,
            trust_key_id: String::new(),
            controller_epoch: 1,
            mdns_enabled: true,
        }
    }
}
