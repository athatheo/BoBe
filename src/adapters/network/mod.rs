use std::collections::HashSet;
use std::net::UdpSocket;

use tracing::{debug, info, warn};

/// Get all local network interface IPs (IPv4 only).
///
/// Uses two strategies:
/// 1. Hostname resolution via DNS
/// 2. UDP connect trick to find the primary LAN IP
pub fn get_local_ips() -> HashSet<String> {
    let mut ips = HashSet::new();

    // Strategy 1: hostname resolution
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

    // Strategy 2: UDP connect trick to find primary LAN IP
    if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("10.255.255.255:1").is_ok() {
            if let Ok(local_addr) = socket.local_addr() {
                ips.insert(local_addr.ip().to_string());
            }
        }
    }

    debug!(count = ips.len(), "network.local_ips_discovered");
    ips
}

/// Information about the local network.
pub struct NetworkInfo;

impl NetworkInfo {
    /// Get the primary LAN IP address.
    pub fn primary_ip() -> Option<String> {
        let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
        socket.connect("10.255.255.255:1").ok()?;
        let addr = socket.local_addr().ok()?;
        Some(addr.ip().to_string())
    }
}

/// Placeholder for mDNS service advertisement.
///
/// Advertises the BoBe service via mDNS so companion devices
/// can discover it on the local network.
pub struct MdnsAnnouncer {
    port: u16,
    enabled: bool,
}

impl MdnsAnnouncer {
    pub fn new(port: u16, enabled: bool) -> Self {
        Self { port, enabled }
    }

    /// Start mDNS advertisement (placeholder — logs and returns).
    pub async fn start(&self) {
        if !self.enabled {
            debug!("mdns.disabled");
            return;
        }
        info!(port = self.port, "mdns.start_placeholder");
        // Full mDNS implementation would use a crate like `mdns-sd` here.
    }

    /// Stop mDNS advertisement.
    pub async fn stop(&self) {
        if !self.enabled {
            return;
        }
        info!("mdns.stop_placeholder");
    }
}
