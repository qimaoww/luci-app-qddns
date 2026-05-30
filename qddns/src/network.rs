use std::process::Command;

/// List network interface names (excluding loopback).
pub fn list_interfaces() -> Vec<String> {
    let output = Command::new("ip")
        .args(["-o", "link", "show"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    let mut interfaces = Vec::new();
    for line in output.lines() {
        let fields: Vec<&str> = line.splitn(3, ':').collect();
        if fields.len() < 2 {
            continue;
        }
        let name = fields[1].trim().split('@').next().unwrap_or("").trim();
        if !name.is_empty() && name != "lo" && !interfaces.contains(&name.to_string()) {
            interfaces.push(name.to_string());
        }
    }
    interfaces
}

/// Entry from IPv6 neighbor table.
#[derive(Debug, Clone)]
pub struct Neighbor6 {
    pub address: String,
    pub interface: String,
    pub mac: String,
}

/// Read IPv6 neighbor table, optionally filtered by interface.
pub fn ipv6_neighbors(interface: Option<&str>) -> Vec<Neighbor6> {
    let mut args = vec!["-6", "neigh", "show"];
    if let Some(iface) = interface {
        args.push("dev");
        args.push(iface);
    }

    let output = Command::new("ip")
        .args(&args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    let mut results = Vec::new();
    for line in output.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 3 {
            continue;
        }
        let address = fields[0];
        let iface = fields[2];
        let mac_pos = fields.iter().position(|&f| f == "lladdr");
        if let Some(pos) = mac_pos {
            if let Some(&mac) = fields.get(pos + 1) {
                results.push(Neighbor6 {
                    address: address.to_string(),
                    interface: iface.to_string(),
                    mac: normalize_mac(mac).unwrap_or_default(),
                });
            }
        }
    }
    results
}

/// Entry from IPv4 neighbor table.
#[derive(Debug, Clone)]
pub struct Neighbor4 {
    pub address: String,
    pub mac: String,
}

/// Read IPv4 neighbor table.
pub fn ipv4_neighbors() -> Vec<Neighbor4> {
    let output = Command::new("ip")
        .args(["-4", "neigh", "show"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    let mut results = Vec::new();
    for line in output.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 3 {
            continue;
        }
        let address = fields[0];
        let mac_pos = fields.iter().position(|&f| f == "lladdr");
        if let Some(pos) = mac_pos {
            if let Some(&mac) = fields.get(pos + 1) {
                results.push(Neighbor4 {
                    address: address.to_string(),
                    mac: normalize_mac(mac).unwrap_or_default(),
                });
            }
        }
    }
    results
}

/// Get /64 public IPv6 prefixes on a given interface from routing table.
pub fn lan_prefixes(interface: &str) -> Vec<String> {
    let output = Command::new("ip")
        .args(["-6", "route", "show", "dev", interface, "proto", "static"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    let mut prefixes = Vec::new();
    for line in output.lines() {
        let prefix = line.split_whitespace().next().unwrap_or("");
        if prefix.ends_with("/64") && (prefix.starts_with('2') || prefix.starts_with('3')) {
            let network = prefix.trim_end_matches("/64")
                .trim_end_matches("::")
                .trim_end_matches(':');
            if !network.is_empty() && !prefixes.contains(&network.to_string()) {
                prefixes.push(network.to_string());
            }
        }
    }
    prefixes
}

/// Ping ff02::1 on interface to refresh neighbor table.
pub fn refresh_ndp(interface: &str) {
    let _ = Command::new("ping6")
        .args(["-c", "1", "-W", "1", "-I", interface, "ff02::1"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

/// Normalize a MAC address to lowercase colon-separated format.
pub fn normalize_mac(mac: &str) -> Option<String> {
    let clean: String = mac.to_lowercase().chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if clean.len() != 12 {
        return None;
    }
    Some(format!(
        "{}:{}:{}:{}:{}:{}",
        &clean[0..2], &clean[2..4], &clean[4..6],
        &clean[6..8], &clean[8..10], &clean[10..12]
    ))
}

/// Check if an IPv6 address is public (global unicast, not documentation).
pub fn is_public_ipv6(addr: &str) -> bool {
    let first = addr.chars().next().unwrap_or('0');
    (first == '2' || first == '3') && addr.contains(':') && !addr.starts_with("2001:db8:")
}

/// Check if an IPv4 address is private (RFC1918).
pub fn is_private_ipv4(addr: &str) -> bool {
    addr.starts_with("10.")
        || addr.starts_with("192.168.")
        || (addr.starts_with("172.") && {
            let second: u8 = addr.split('.').nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
            (16..=31).contains(&second)
        })
}
