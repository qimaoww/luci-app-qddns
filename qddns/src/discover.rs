use std::process::Command;
use std::thread;

use crate::network::{ipv6_neighbors, lan_prefixes, refresh_ndp};

/// Convert MAC to EUI-64 interface identifier (without leading zeros, matching kernel format).
pub fn mac_to_eui64(mac: &str) -> Option<String> {
    let parts: Vec<&str> = mac.split(':').collect();
    if parts.len() != 6 {
        return None;
    }
    let bytes: Vec<u8> = parts
        .iter()
        .filter_map(|p| u8::from_str_radix(p, 16).ok())
        .collect();
    if bytes.len() != 6 {
        return None;
    }
    let b0 = bytes[0] ^ 0x02;
    let w1 = ((b0 as u16) << 8) | (bytes[1] as u16);
    let w2 = ((bytes[2] as u16) << 8) | 0xff;
    let w3: u16 = (0xfe << 8) | (bytes[3] as u16);
    let w4 = ((bytes[4] as u16) << 8) | (bytes[5] as u16);
    Some(format!("{:x}:{:x}:{:x}:{:x}", w1, w2, w3, w4))
}

/// Result of SLAAC address discovery.
#[derive(Debug, Clone)]
pub struct DiscoveredAddress {
    pub mac: String,
    pub address: String,
}

/// Discover SLAAC addresses by computing EUI-64 from known MACs and LAN prefixes,
/// then verifying reachability with parallel ping.
pub fn discover_slaac(lan_interface: &str) -> Vec<DiscoveredAddress> {
    if lan_interface.is_empty() {
        return Vec::new();
    }

    // Refresh neighbor table first
    refresh_ndp(lan_interface);
    // Small delay to let responses arrive
    std::thread::sleep(std::time::Duration::from_millis(200));

    let prefixes = lan_prefixes(lan_interface);
    if prefixes.is_empty() {
        return Vec::new();
    }

    // Collect known MACs from neighbor table
    let neighbors = ipv6_neighbors(Some(lan_interface));
    let mut macs: Vec<String> = Vec::new();
    for n in &neighbors {
        if !n.mac.is_empty() && !macs.contains(&n.mac) {
            macs.push(n.mac.clone());
        }
    }

    // Generate targets
    let mut targets: Vec<(String, String)> = Vec::new();
    for mac in &macs {
        if let Some(suffix) = mac_to_eui64(mac) {
            for prefix in &prefixes {
                let addr = format!("{}:{}", prefix, suffix);
                targets.push((mac.clone(), addr));
            }
        }
    }

    // Parallel ping verification
    let iface = lan_interface.to_string();
    let handles: Vec<_> = targets
        .into_iter()
        .map(|(mac, addr)| {
            let iface = iface.clone();
            thread::spawn(move || {
                let reachable = Command::new("ping6")
                    .args(["-c", "1", "-W", "1", "-I", &iface, &addr])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);
                (mac, addr, reachable)
            })
        })
        .collect();

    let mut results = Vec::new();
    for handle in handles {
        if let Ok((mac, addr, true)) = handle.join() {
            results.push(DiscoveredAddress { mac, address: addr });
        }
    }
    results
}
