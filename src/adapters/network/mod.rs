use std::collections::HashSet;
use std::net::UdpSocket;

use tracing::{debug, error, info, warn};

/// Get all local network interface IPs (IPv4 only).
pub fn get_local_ips() -> HashSet<String> {
    let mut ips = HashSet::new();

    if let Ok(hostname) = hostname::get() {
        let hostname_str = hostname.to_string_lossy().to_string();
        if let Ok(addrs) = std::net::ToSocketAddrs::to_socket_addrs(&(hostname_str.as_str(), 0)) {
            for addr in addrs {
                if addr.is_ipv4() {
                    ips.insert(addr.ip().to_string());
                }
            }
        }
    }

    if let Ok(socket) = UdpSocket::bind("0.0.0.0:0")
        && socket.connect("10.255.255.255:1").is_ok()
            && let Ok(local_addr) = socket.local_addr() {
                ips.insert(local_addr.ip().to_string());
            }

    debug!(count = ips.len(), "network.local_ips_discovered");
    ips
}

/// Information about the local network.
pub struct NetworkInfo;

impl NetworkInfo {
    pub fn primary_ip() -> Option<String> {
        let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
        socket.connect("10.255.255.255:1").ok()?;
        let addr = socket.local_addr().ok()?;
        Some(addr.ip().to_string())
    }
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

        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "bobe".into());

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
                    Ok(_) => {
                        info!(port = self.port, service_type = SERVICE_TYPE, "mdns.advertising");
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
