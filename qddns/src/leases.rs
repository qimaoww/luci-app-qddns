use std::collections::BTreeMap;
use std::fs;

use crate::discover::discover_slaac;
use crate::network::{ipv4_neighbors, ipv6_neighbors, is_private_ipv4, is_public_ipv6, normalize_mac};

const DHCPV4_LEASE_FILE: &str = "/tmp/dhcp.leases";
const DHCPV6_LEASE_FILE: &str = "/tmp/odhcpd.leases";
const MAX_ENTRIES: usize = 64;
const MAX_PREFIXES_PER_ENTRY: usize = 8;

/// A host entry collected from lease files and neighbor tables.
#[derive(Debug, Clone)]
pub struct HostEntry {
    pub mac: String,
    pub hostname: Option<String>,
    pub duid: Option<String>,
    pub iaid: Option<String>,
    pub ipv4: Vec<String>,
    pub prefixes: Vec<String>,
    pub interfaces: Vec<String>,
    pub lease_file: Option<String>,
}

impl HostEntry {
    fn new(mac: String) -> Self {
        Self {
            mac,
            hostname: None,
            duid: None,
            iaid: None,
            ipv4: Vec::new(),
            prefixes: Vec::new(),
            interfaces: Vec::new(),
            lease_file: None,
        }
    }

    fn push_prefix(&mut self, prefix: &str) {
        if self.prefixes.len() < MAX_PREFIXES_PER_ENTRY && !self.prefixes.contains(&prefix.to_string()) {
            self.prefixes.push(prefix.to_string());
        }
    }

    fn push_interface(&mut self, iface: &str) {
        if !iface.is_empty() && !self.interfaces.contains(&iface.to_string()) {
            self.interfaces.push(iface.to_string());
        }
    }

    fn push_ipv4(&mut self, addr: &str) {
        if !addr.is_empty() && !self.ipv4.contains(&addr.to_string()) {
            self.ipv4.push(addr.to_string());
        }
    }
}

/// Extract MAC from DUID (last 12 hex chars).
fn duid_to_mac(duid: &str) -> Option<String> {
    let clean: String = duid.to_lowercase().chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if clean.len() < 12 {
        return None;
    }
    normalize_mac(&clean[clean.len() - 12..])
}

fn strip_lan_suffix(hostname: &str) -> &str {
    hostname.strip_suffix(".lan").unwrap_or(hostname)
}

/// Collect entries from /tmp/dhcp.leases (dnsmasq DHCPv4).
fn add_dhcpv4_entries(entries: &mut BTreeMap<String, HostEntry>) {
    let content = fs::read_to_string(DHCPV4_LEASE_FILE).unwrap_or_default();
    for line in content.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 4 {
            continue;
        }
        let mac = match normalize_mac(fields[1]) {
            Some(m) => m,
            None => continue,
        };
        let entry = entries.entry(mac.clone()).or_insert_with(|| HostEntry::new(mac));
        if fields[3] != "*" && !fields[3].is_empty() {
            entry.hostname = entry.hostname.clone().or_else(|| Some(strip_lan_suffix(fields[3]).to_string()));
        }
        if is_private_ipv4(fields[2]) {
            entry.push_ipv4(fields[2]);
        }
    }
}

/// Collect entries from /tmp/odhcpd.leases (odhcpd DHCPv6).
fn add_dhcpv6_entries(entries: &mut BTreeMap<String, HostEntry>) {
    let content = fs::read_to_string(DHCPV6_LEASE_FILE).unwrap_or_default();
    for line in content.lines() {
        if entries.len() >= MAX_ENTRIES {
            break;
        }
        let line = line.trim();
        if !line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 9 {
            continue;
        }

        // fields: # expiry iface duid iaid hostname ... prefixes
        let mac = match duid_to_mac(fields[2]) {
            Some(m) => m,
            None => continue,
        };

        let mut prefixes = Vec::new();
        for field in &fields[8..] {
            let raw = field.split('/').next().unwrap_or("");
            if is_public_ipv6(raw) && prefixes.len() < MAX_PREFIXES_PER_ENTRY {
                prefixes.push(field.to_string());
            }
        }
        if prefixes.is_empty() {
            continue;
        }

        let entry = entries.entry(mac.clone()).or_insert_with(|| HostEntry::new(mac));
        entry.duid = Some(fields[2].to_string());
        entry.iaid = Some(fields[3].to_string());
        entry.push_interface(fields[1]);
        if entry.hostname.is_none() && !fields[4].is_empty() && fields[4] != "-" {
            entry.hostname = Some(fields[4].to_string());
        }
        entry.lease_file = Some(DHCPV6_LEASE_FILE.to_string());
        for prefix in prefixes {
            entry.push_prefix(&prefix);
        }
    }
}

/// Add IPv6 addresses from neighbor table.
fn add_ndp_entries(entries: &mut BTreeMap<String, HostEntry>, lan_interface: Option<&str>) {
    let neighbors = ipv6_neighbors(lan_interface);
    for n in neighbors {
        if n.mac.is_empty() || !is_public_ipv6(&n.address) {
            continue;
        }
        let entry = entries.entry(n.mac.clone()).or_insert_with(|| HostEntry::new(n.mac));
        entry.push_interface(&n.interface);
        entry.push_prefix(&format!("{}/128", n.address));
    }
}

/// Add private IPv4 from neighbor table to existing entries.
fn add_ipv4_entries(entries: &mut BTreeMap<String, HostEntry>) {
    let neighbors = ipv4_neighbors();
    for n in neighbors {
        if n.mac.is_empty() || !is_private_ipv4(&n.address) {
            continue;
        }
        if let Some(entry) = entries.get_mut(&n.mac) {
            entry.push_ipv4(&n.address);
        }
    }
}

/// Add SLAAC-discovered addresses.
fn add_slaac_entries(entries: &mut BTreeMap<String, HostEntry>, lan_interface: &str) {
    let discovered = discover_slaac(lan_interface);
    for d in discovered {
        if let Some(entry) = entries.get_mut(&d.mac) {
            entry.push_prefix(&format!("{}/128", d.address));
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaseMode {
    Duid,
    Mac,
}

/// Collect all host entries and return as a list filtered by mode.
pub fn list_leases(mode: LeaseMode, lan_interface: Option<&str>) -> Vec<HostEntry> {
    let mut entries: BTreeMap<String, HostEntry> = BTreeMap::new();

    add_dhcpv6_entries(&mut entries);
    add_dhcpv4_entries(&mut entries);
    add_ndp_entries(&mut entries, lan_interface);
    add_ipv4_entries(&mut entries);

    if let Some(iface) = lan_interface {
        if !iface.is_empty() {
            add_slaac_entries(&mut entries, iface);
        }
    }

    entries
        .into_values()
        .filter(|e| !e.prefixes.is_empty())
        .map(|mut e| {
            if mode == LeaseMode::Mac {
                e.duid = None;
                e.iaid = None;
                e.lease_file = None;
            }
            e
        })
        .collect()
}
