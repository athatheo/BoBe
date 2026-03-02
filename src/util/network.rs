use tracing::{debug, error, info, warn};

/// Retrieve the system hostname via `libc::gethostname`, falling back to `"bobe"`.
#[allow(unsafe_code)]
fn get_hostname() -> String {
    let mut buf = [0u8; 256];
    // SAFETY: gethostname writes into a fixed-size buffer we own.
    let ret = unsafe { libc::gethostname(buf.as_mut_ptr().cast::<libc::c_char>(), buf.len()) };
    if ret != 0 {
        return "bobe".into();
    }
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).into_owned()
}

const SERVICE_TYPE: &str = "_bobe._tcp.local.";

/// mDNS service advertisement for LAN device discovery.
///
/// Advertises the BoBe service so companion devices (e.g. ESP32)
/// can discover it without hardcoding IP addresses.
pub struct MdnsAnnouncer {
    port: u16,
    enabled: bool,
    daemon: tokio::sync::Mutex<Option<mdns_sd::ServiceDaemon>>,
}

impl MdnsAnnouncer {
    pub fn new(port: u16, enabled: bool) -> Self {
        Self {
            port,
            enabled,
            daemon: tokio::sync::Mutex::new(None),
        }
    }

    /// Start mDNS advertisement.
    pub async fn start(&self) {
        if !self.enabled {
            debug!("mdns.disabled");
            return;
        }

        let hostname = get_hostname();

        let instance_name = format!("BoBe on {hostname}");

        match mdns_sd::ServiceDaemon::new() {
            Ok(daemon) => {
                let service_info = match mdns_sd::ServiceInfo::new(
                    SERVICE_TYPE,
                    &instance_name,
                    &format!("{hostname}.local."),
                    "",
                    self.port,
                    None,
                ) {
                    Ok(info) => info,
                    Err(e) => {
                        warn!(error = %e, "mdns.service_info_failed");
                        return;
                    }
                };

                match daemon.register(service_info) {
                    Ok(()) => {
                        info!(
                            port = self.port,
                            service_type = SERVICE_TYPE,
                            "mdns.advertising"
                        );
                        *self.daemon.lock().await = Some(daemon);
                    }
                    Err(e) => {
                        error!(error = %e, "mdns.register_failed");
                    }
                }
            }
            Err(e) => {
                warn!(error = %e, "mdns.daemon_creation_failed");
            }
        }
    }

    /// Stop mDNS advertisement.
    pub async fn stop(&self) {
        if let Some(daemon) = self.daemon.lock().await.take() {
            if let Err(e) = daemon.shutdown() {
                warn!(error = %e, "mdns.shutdown_error");
            } else {
                info!("mdns.stopped");
            }
        }
    }
}
