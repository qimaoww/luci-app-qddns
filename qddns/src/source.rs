use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::net::{IpAddr, Ipv6Addr};
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::config::{AddressFamily, SourceConfig, SourceKind};
use crate::error::{Error, Result};
use crate::http::{HttpClient, HttpRequest, RetryPolicy};

const DHCPV6_LEASE_MAX_BYTES: u64 = 262_144;
const SOURCE_COMMAND_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceResolution {
    pub address: IpAddr,
    pub family: String,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Ipv6Prefix {
    address: Ipv6Addr,
    prefix_len: u8,
}

impl Ipv6Prefix {
    fn contains(self, address: &Ipv6Addr) -> bool {
        let mask = ipv6_mask(self.prefix_len);
        ipv6_to_u128(self.address) & mask == ipv6_to_u128(*address) & mask
    }
}

pub fn resolve_source(source: &SourceConfig) -> Result<SourceResolution> {
    let http = HttpClient::from_timeout_secs(15);
    resolve_source_with_http(source, &http)
}

pub fn resolve_source_with_http(
    source: &SourceConfig,
    http: &HttpClient,
) -> Result<SourceResolution> {
    match &source.kind {
        SourceKind::LocalAddr { address, .. } => resolve_local_addr(source, address.as_deref()),
        SourceKind::Script { script, .. } => resolve_script(source, script.as_deref()),
        SourceKind::PublicProbe { probe_url, .. } => {
            resolve_public_probe(source, probe_url.as_deref(), http)
        }
        SourceKind::Interface { family, interface } => {
            resolve_interface(source, *family, interface.as_deref())
        }
        SourceKind::Dhcpv6Duid {
            duid,
            iaid,
            interface,
            lease_file,
            prefix_filter,
            ..
        } => resolve_dhcpv6_duid(
            source,
            duid.as_deref(),
            iaid.as_deref(),
            interface.as_deref(),
            lease_file.as_deref(),
            prefix_filter.as_deref(),
        ),
        SourceKind::Dhcpv6Mac {
            mac,
            interface,
            lease_file,
            prefix_filter,
            ..
        } => resolve_dhcpv6_mac(
            source,
            mac.as_deref(),
            interface.as_deref(),
            lease_file.as_deref(),
            prefix_filter.as_deref(),
        ),
    }
}

fn resolve_local_addr(source: &SourceConfig, address: Option<&str>) -> Result<SourceResolution> {
    let address = address
        .ok_or_else(|| Error::new(format!("source '{}' missing address", source.name)))?
        .parse::<IpAddr>()
        .map_err(|err| Error::new(format!("invalid address: {err}")))?;

    Ok(SourceResolution {
        family: if address.is_ipv4() { "ipv4" } else { "ipv6" }.into(),
        detail: "configured local address".into(),
        address,
    })
}

fn resolve_dhcpv6_duid(
    source: &SourceConfig,
    duid: Option<&str>,
    iaid: Option<&str>,
    interface: Option<&str>,
    lease_file: Option<&str>,
    prefix_filter: Option<&str>,
) -> Result<SourceResolution> {
    let duid = duid.ok_or_else(|| Error::new(format!("source '{}' missing duid", source.name)))?;
    let iaid = iaid.ok_or_else(|| Error::new(format!("source '{}' missing iaid", source.name)))?;
    let ifaces = required_dhcpv6_interfaces(source, interface)?;
    let explicit_lease_file = lease_file.map(str::trim).filter(|value| !value.is_empty());
    let lease_file = explicit_lease_file.unwrap_or("/tmp/odhcpd.leases");

    let content = read_dhcpv6_lease_file(lease_file)?;
    let wan_source_prefixes = interfaces_wan_source_ipv6_prefixes(source, &ifaces)?;

    let mut matches = Vec::<IpAddr>::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('#') {
            continue;
        }

        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.len() < 9 {
            continue;
        }
        if fields[2] != duid || fields[3] != iaid {
            continue;
        }

        collect_public_ipv6_candidates(&fields, &mut matches);
    }

    if matches.is_empty() {
        return Err(Error::new(format!(
            "no IPv6 lease found for DUID '{duid}' and IAID '{iaid}'"
        )));
    }

    let selected = select_lease_address(
        &matches,
        &wan_source_prefixes,
        prefix_filter,
        &format!("DUID '{duid}'"),
    )?;

    Ok(SourceResolution {
        address: selected,
        family: "ipv6".into(),
        detail: format!(
            "resolved from {lease_file} via interface {}",
            format_interface_names(&ifaces)
        ),
    })
}

fn resolve_dhcpv6_mac(
    source: &SourceConfig,
    mac: Option<&str>,
    interface: Option<&str>,
    lease_file: Option<&str>,
    prefix_filter: Option<&str>,
) -> Result<SourceResolution> {
    let mac = mac.ok_or_else(|| Error::new(format!("source '{}' missing mac", source.name)))?;
    let normalized_mac = normalize_mac(mac)?;
    let ifaces = required_dhcpv6_interfaces(source, interface)?;
    let explicit_lease_file = lease_file.map(str::trim).filter(|value| !value.is_empty());
    let lease_file = explicit_lease_file.unwrap_or("/tmp/odhcpd.leases");

    let mut matches = Vec::<IpAddr>::new();

    let lease_error = match read_dhcpv6_lease_file(lease_file) {
        Ok(content) => {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || !line.starts_with('#') {
                    continue;
                }

                let fields = line.split_whitespace().collect::<Vec<_>>();
                if fields.len() < 9 {
                    continue;
                }
                let Some(duid_mac) = duid_link_layer_mac(fields[2]) else {
                    continue;
                };
                if duid_mac != normalized_mac {
                    continue;
                }

                collect_public_ipv6_candidates(&fields, &mut matches);
            }
            None
        }
        Err(err) if explicit_lease_file.is_some() => return Err(err),
        Err(err) => Some(err),
    };
    let wan_source_prefixes = interfaces_wan_source_ipv6_prefixes(source, &ifaces)?;

    collect_ndp_ipv6_candidates(&normalized_mac, &mut matches);

    if matches.is_empty() {
        if let Some(err) = lease_error {
            return Err(err);
        }

        return Err(Error::new(format!(
            "no public IPv6 address found for MAC '{}'",
            format_mac(&normalized_mac)
        )));
    }

    let selected = select_lease_address(
        &matches,
        &wan_source_prefixes,
        prefix_filter,
        &format!("MAC '{}'", format_mac(&normalized_mac)),
    )?;

    Ok(SourceResolution {
        address: selected,
        family: "ipv6".into(),
        detail: format!(
            "resolved by MAC from LAN host tables and {lease_file} via interface {}",
            format_interface_names(&ifaces)
        ),
    })
}

fn read_dhcpv6_lease_file(lease_file: &str) -> Result<String> {
    let path = Path::new(lease_file);
    if !path.is_absolute() {
        return Err(Error::new(format!(
            "lease file '{lease_file}' must be an absolute path"
        )));
    }
    let path = fs::canonicalize(path)
        .map_err(|err| Error::new(format!("failed to read lease file '{lease_file}': {err}")))?;
    if path.starts_with("/dev") || path.starts_with("/proc") || path.starts_with("/sys") {
        return Err(Error::new(format!(
            "lease file '{lease_file}' is not allowed"
        )));
    }

    let metadata = fs::metadata(&path)
        .map_err(|err| Error::new(format!("failed to read lease file '{lease_file}': {err}")))?;
    if !metadata.file_type().is_file() {
        return Err(Error::new(format!(
            "lease file '{lease_file}' must be a regular file"
        )));
    }

    let mut content = String::new();
    let mut file = fs::File::open(&path)
        .map_err(|err| Error::new(format!("failed to read lease file '{lease_file}': {err}")))?;
    let opened_metadata = file
        .metadata()
        .map_err(|err| Error::new(format!("failed to read lease file '{lease_file}': {err}")))?;
    if !opened_metadata.file_type().is_file() {
        return Err(Error::new(format!(
            "lease file '{lease_file}' must be a regular file"
        )));
    }
    file.by_ref()
        .take(DHCPV6_LEASE_MAX_BYTES + 1)
        .read_to_string(&mut content)
        .map_err(|err| Error::new(format!("failed to read lease file '{lease_file}': {err}")))?;
    if content.len() as u64 > DHCPV6_LEASE_MAX_BYTES {
        return Err(Error::new(format!(
            "lease file '{lease_file}' exceeds {DHCPV6_LEASE_MAX_BYTES} bytes"
        )));
    }

    Ok(content)
}

fn collect_public_ipv6_candidates(fields: &[&str], matches: &mut Vec<IpAddr>) {
    for candidate in fields.iter().skip(8) {
        let raw = candidate.split('/').next().unwrap_or("");
        push_public_ipv6(raw, matches);
    }
}

fn collect_ndp_ipv6_candidates(normalized_mac: &str, matches: &mut Vec<IpAddr>) {
    let Ok(output) = command_output_with_timeout(
        Command::new("ip").args(["-6", "neigh", "show"]),
        SOURCE_COMMAND_TIMEOUT,
        "ip -6 neigh show",
    ) else {
        return;
    };
    if !output.status.success() {
        return;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    collect_ndp_ipv6_candidates_from_output(&stdout, normalized_mac, matches);
}

fn collect_ndp_ipv6_candidates_from_output(
    output: &str,
    normalized_mac: &str,
    matches: &mut Vec<IpAddr>,
) {
    for line in output.lines() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        let Some(address) = fields.first() else {
            continue;
        };
        let Some(lladdr_index) = fields.iter().position(|field| *field == "lladdr") else {
            continue;
        };
        let Some(mac) = fields.get(lladdr_index + 1) else {
            continue;
        };
        if normalize_mac(mac).ok().as_deref() != Some(normalized_mac) {
            continue;
        }

        push_public_ipv6(address, matches);
    }
}

fn push_public_ipv6(raw: &str, matches: &mut Vec<IpAddr>) {
    let raw = raw.split('/').next().unwrap_or("");
    let Ok(ip @ IpAddr::V6(ipv6)) = raw.parse::<IpAddr>() else {
        return;
    };
    if !is_public_ipv6(&ipv6) || matches.contains(&ip) {
        return;
    }

    matches.push(ip);
}

fn is_public_ipv6(ip: &std::net::Ipv6Addr) -> bool {
    let segments = ip.segments();
    let first = segments[0];

    (0x2000..=0x3fff).contains(&first) && !(segments[0] == 0x2001 && segments[1] == 0x0db8)
}

fn parse_interface_names(interface: Option<&str>) -> Vec<String> {
    interface
        .unwrap_or("")
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .fold(Vec::<String>::new(), |mut names, value| {
            if !names.iter().any(|name| name == value) {
                names.push(value.to_string());
            }
            names
        })
}

fn required_dhcpv6_interfaces(
    source: &SourceConfig,
    interface: Option<&str>,
) -> Result<Vec<String>> {
    let ifaces = parse_interface_names(interface);
    if ifaces.is_empty() {
        return Err(Error::new(format!(
            "source '{}' missing interface",
            source.name
        )));
    }

    for iface in &ifaces {
        validate_interface_name(iface)?;
    }

    Ok(ifaces)
}

fn format_interface_names(ifaces: &[String]) -> String {
    ifaces.join(", ")
}

fn interfaces_wan_source_ipv6_prefixes(
    source: &SourceConfig,
    ifaces: &[String],
) -> Result<Vec<Ipv6Prefix>> {
    let mut prefixes = Vec::new();
    for iface in ifaces {
        for prefix in interface_wan_source_ipv6_prefixes(iface)? {
            if !prefixes.contains(&prefix) {
                prefixes.push(prefix);
            }
        }
    }
    if prefixes.is_empty() {
        return Err(Error::new(format!(
            "selected WAN/upstream source prefix set for source '{}' is empty; interfaces {} have no public IPv6 route source prefix",
            source.name,
            format_interface_names(ifaces)
        )));
    }

    Ok(prefixes)
}

fn interface_wan_source_ipv6_prefixes(iface: &str) -> Result<Vec<Ipv6Prefix>> {
    interface_route_source_ipv6_prefixes(iface)
}

fn interface_route_source_ipv6_prefixes(iface: &str) -> Result<Vec<Ipv6Prefix>> {
    let output = command_output_with_timeout(
        Command::new("ip").args(["-6", "route", "show", "table", "all"]),
        SOURCE_COMMAND_TIMEOUT,
        "ip -6 route show table all",
    )
    .map_err(|err| {
        Error::new(format!(
            "failed to inspect IPv6 route source prefixes for interface '{iface}': {err}"
        ))
    })?;
    if !output.status.success() {
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_interface_route_source_ipv6_prefixes(&stdout, iface))
}

fn parse_interface_route_source_ipv6_prefixes(output: &str, iface: &str) -> Vec<Ipv6Prefix> {
    let mut prefixes = Vec::new();

    for line in output.lines() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        let Some(dev_index) = fields.iter().position(|field| *field == "dev") else {
            continue;
        };
        if fields.get(dev_index + 1).copied() != Some(iface) {
            continue;
        }

        let Some(from_index) = fields.iter().position(|field| *field == "from") else {
            continue;
        };
        let Some(prefix) = fields
            .get(from_index + 1)
            .and_then(|value| parse_ipv6_prefix(value))
        else {
            continue;
        };
        if prefix.prefix_len == 0 || !is_public_ipv6(&prefix.address) {
            continue;
        }

        push_unique_prefix(&mut prefixes, prefix);
    }

    prefixes
}

fn push_unique_prefix(prefixes: &mut Vec<Ipv6Prefix>, prefix: Ipv6Prefix) {
    if !prefixes.contains(&prefix) {
        prefixes.push(prefix);
    }
}

fn parse_ipv6_prefix(input: &str) -> Option<Ipv6Prefix> {
    let (address, prefix_len) = input.split_once('/')?;
    let address = address.parse::<Ipv6Addr>().ok()?;
    let prefix_len = prefix_len.parse::<u8>().ok()?;
    if prefix_len > 128 {
        return None;
    }

    Some(Ipv6Prefix {
        address,
        prefix_len,
    })
}

fn parse_prefix_filter(filter: &str) -> Result<Ipv6Prefix> {
    let filter = filter.trim();
    if filter.is_empty() {
        return Err(Error::new("prefix_filter must not be empty"));
    }

    if let Some(prefix) = parse_ipv6_prefix(filter) {
        return Ok(prefix);
    }

    if let Ok(address) = filter.parse::<Ipv6Addr>() {
        return Ok(Ipv6Prefix {
            address,
            prefix_len: 128,
        });
    }

    parse_hextet_prefix_filter(filter)
        .ok_or_else(|| Error::new(format!("invalid prefix_filter '{filter}'")))
}

fn parse_hextet_prefix_filter(filter: &str) -> Option<Ipv6Prefix> {
    let filter = filter.trim_end_matches(':');
    if filter.is_empty() {
        return None;
    }

    let parts = filter.split(':').collect::<Vec<_>>();
    if parts.len() > 8 || parts.iter().any(|part| part.is_empty() || part.len() > 4) {
        return None;
    }

    let mut segments = [0u16; 8];
    for (index, part) in parts.iter().enumerate() {
        segments[index] = u16::from_str_radix(part, 16).ok()?;
    }

    Some(Ipv6Prefix {
        address: Ipv6Addr::new(
            segments[0],
            segments[1],
            segments[2],
            segments[3],
            segments[4],
            segments[5],
            segments[6],
            segments[7],
        ),
        prefix_len: (parts.len() * 16) as u8,
    })
}

fn ipv6_to_u128(address: Ipv6Addr) -> u128 {
    u128::from_be_bytes(address.octets())
}

fn ipv6_mask(prefix_len: u8) -> u128 {
    if prefix_len == 0 {
        0
    } else {
        u128::MAX << (128 - prefix_len)
    }
}

fn select_lease_address(
    matches: &[IpAddr],
    wan_source_prefixes: &[Ipv6Prefix],
    prefix_filter: Option<&str>,
    subject: &str,
) -> Result<IpAddr> {
    let mut wan_matches = Vec::new();
    for address in matches {
        let IpAddr::V6(ipv6) = address else {
            continue;
        };
        if !is_public_ipv6(ipv6) {
            continue;
        }
        if wan_source_prefixes
            .iter()
            .any(|prefix| prefix.contains(ipv6))
        {
            wan_matches.push(*address);
        }
    }

    if wan_matches.is_empty() {
        return Err(Error::new(format!(
            "no IPv6 lease matched WAN/upstream source prefix for {subject}"
        )));
    }

    let selected = if let Some(prefix) = prefix_filter.filter(|value| !value.trim().is_empty()) {
        let prefix = parse_prefix_filter(prefix)?;
        let narrowed = wan_matches
            .iter()
            .copied()
            .filter(|address| match address {
                IpAddr::V6(ipv6) => prefix.contains(ipv6),
                IpAddr::V4(_) => false,
            })
            .collect::<Vec<_>>();
        if narrowed.is_empty() {
            return Err(Error::new(format!(
                "no IPv6 lease matched prefix_filter after WAN/upstream source prefix for {subject}"
            )));
        }
        narrowed
    } else {
        wan_matches
    };

    selected.first().copied().ok_or_else(|| {
        Error::new(format!(
            "no IPv6 lease matched WAN/upstream source prefix for {subject}"
        ))
    })
}

fn normalize_mac(mac: &str) -> Result<String> {
    let hex = mac
        .bytes()
        .filter(|byte| *byte != b':' && *byte != b'-')
        .map(char::from)
        .collect::<String>()
        .to_ascii_lowercase();

    if hex.len() == 12 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(hex)
    } else {
        Err(Error::new(format!("invalid MAC address '{mac}'")))
    }
}

fn duid_link_layer_mac(duid: &str) -> Option<String> {
    let hex = duid.to_ascii_lowercase();
    if hex.len() >= 12 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Some(hex[hex.len() - 12..].to_string())
    } else {
        None
    }
}

fn format_mac(mac: &str) -> String {
    mac.as_bytes()
        .chunks(2)
        .map(|chunk| std::str::from_utf8(chunk).unwrap_or(""))
        .collect::<Vec<_>>()
        .join(":")
}

fn resolve_script(source: &SourceConfig, script: Option<&str>) -> Result<SourceResolution> {
    let script =
        script.ok_or_else(|| Error::new(format!("source '{}' missing script", source.name)))?;
    if !script.starts_with('/') {
        return Err(Error::new(format!(
            "source '{}' script must be an absolute path",
            source.name
        )));
    }
    let mut command = Command::new(script);
    let output = command_output_with_timeout(&mut command, SOURCE_COMMAND_TIMEOUT, "script source")
        .map_err(|err| Error::new(format!("failed to execute script: {err}")))?;
    if !output.status.success() {
        return Err(Error::new(format!(
            "script source '{}' exited with status {}",
            source.name, output.status
        )));
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    parse_address_output(&stdout, "script")
}

fn resolve_public_probe(
    source: &SourceConfig,
    probe_url: Option<&str>,
    http: &HttpClient,
) -> Result<SourceResolution> {
    let probe_url = probe_url
        .ok_or_else(|| Error::new(format!("source '{}' missing probe_url", source.name)))?;

    let body = if probe_url.starts_with("http://") || probe_url.starts_with("https://") {
        http.execute(
            &HttpRequest {
                method: "GET".into(),
                url: probe_url.into(),
                headers: BTreeMap::new(),
                body: String::new(),
            },
            RetryPolicy::none(),
        )?
        .body
    } else {
        return Err(Error::new(format!(
            "source '{}' has unsupported probe URL scheme",
            source.name
        )));
    };

    let candidate = find_ip_in_text(&body).ok_or_else(|| {
        Error::new(format!(
            "no IP address found in probe response for '{}'",
            source.name
        ))
    })?;
    parse_address_output(candidate, "public probe")
}

fn resolve_interface(
    source: &SourceConfig,
    family: Option<AddressFamily>,
    interface: Option<&str>,
) -> Result<SourceResolution> {
    let ifaces = required_dhcpv6_interfaces(source, interface)?;

    for iface in &ifaces {
        if iface == "lo" {
            let address = match family {
                Some(AddressFamily::Ipv6) => "::1".parse::<IpAddr>().unwrap(),
                _ => "127.0.0.1".parse::<IpAddr>().unwrap(),
            };
            return Ok(SourceResolution {
                family: if address.is_ipv4() { "ipv4" } else { "ipv6" }.into(),
                detail: format!("loopback fallback for interface {iface}"),
                address,
            });
        }

        let output = command_output_with_timeout(
            Command::new("ip").args(["addr", "show", "dev", iface]),
            SOURCE_COMMAND_TIMEOUT,
            &format!("ip addr show dev {iface}"),
        )
        .map_err(|err| Error::new(format!("failed to inspect interface '{iface}': {err}")))?;
        if !output.status.success() {
            return Err(Error::new(format!(
                "unable to inspect interface '{}'",
                iface
            )));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(address) = parse_interface_address(&stdout, family) {
            return parse_address_output(&address.to_string(), "interface");
        }
    }

    Err(Error::new(format!(
        "interfaces '{}' have no address",
        format_interface_names(&ifaces)
    )))
}

fn command_output_with_timeout(
    command: &mut Command,
    timeout: Duration,
    description: &str,
) -> Result<Output> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| Error::new(format!("{description} failed to start: {err}")))?;
    let start = Instant::now();

    loop {
        if child
            .try_wait()
            .map_err(|err| Error::new(format!("{description} failed to wait: {err}")))?
            .is_some()
        {
            return child
                .wait_with_output()
                .map_err(|err| Error::new(format!("{description} failed to read output: {err}")));
        }

        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::new(format!(
                "{description} timed out after {}s",
                timeout.as_secs().max(1)
            )));
        }

        thread::sleep(Duration::from_millis(20));
    }
}

fn validate_interface_name(iface: &str) -> Result<()> {
    if iface.is_empty() || iface.len() > 64 {
        return Err(Error::new("invalid interface name"));
    }
    if iface.bytes().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b':' | b'@' | b'-')
    }) {
        Ok(())
    } else {
        Err(Error::new("invalid interface name"))
    }
}

fn parse_interface_address(output: &str, family: Option<AddressFamily>) -> Option<IpAddr> {
    for line in output.lines() {
        let line = line.trim_start();
        let rest = if let Some(rest) = line.strip_prefix("inet ") {
            rest
        } else if let Some(rest) = line.strip_prefix("inet6 ") {
            rest
        } else {
            continue;
        };
        let address_text = rest.split_whitespace().next()?.split('/').next()?;
        let address = address_text.parse::<IpAddr>().ok()?;
        match family {
            Some(AddressFamily::Ipv4) if !address.is_ipv4() => continue,
            Some(AddressFamily::Ipv6) if !address.is_ipv6() => continue,
            _ => return Some(address),
        }
    }
    None
}

fn parse_address_output(output: &str, detail: &str) -> Result<SourceResolution> {
    let address = output
        .parse::<IpAddr>()
        .map_err(|err| Error::new(format!("invalid IP output '{output}': {err}")))?;
    Ok(SourceResolution {
        family: if address.is_ipv4() { "ipv4" } else { "ipv6" }.into(),
        detail: detail.into(),
        address,
    })
}

fn find_ip_in_text(text: &str) -> Option<&str> {
    for token in
        text.split(|c: char| c.is_whitespace() || [',', ';', '[', ']', '(', ')'].contains(&c))
    {
        if token.parse::<IpAddr>().is_ok() {
            return Some(token);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wan_source_prefix_accepts_matching_public_ipv6() {
        let prefixes = parse_interface_route_source_ipv6_prefixes(
            "default from 240e:3b2:4e8a:70a0::/64 dev pppoe-wan proto static metric 512 pref medium\n",
            "pppoe-wan",
        );
        let matches = vec![
            "2409:8a55:4e26:6980::30".parse::<IpAddr>().unwrap(),
            "240E:03B2:4E8A:70A0::30".parse::<IpAddr>().unwrap(),
        ];

        let selected = select_lease_address(&matches, &prefixes, None, "MAC").unwrap();

        assert_eq!(
            selected,
            "240e:3b2:4e8a:70a0::30".parse::<IpAddr>().unwrap()
        );
    }

    #[test]
    fn wan_route_from_prefix_accepts_delegated_pd_candidate() {
        let prefixes = parse_interface_route_source_ipv6_prefixes(
            "default from 240e:3b2:4e8a:7000::/60 dev pppoe-wan proto static metric 512 pref medium\n\
             default from 240e:3b2:4e8a:7000::/60 dev pppoe-wan_cmcc proto static metric 512 pref medium\n\
             default from all dev pppoe-wan proto static metric 512 pref medium\n",
            "pppoe-wan",
        );
        let matches = vec![
            "240e:3b2:4e8a:7001::30".parse::<IpAddr>().unwrap(),
            "240e:3b2:4e8a:7010::30".parse::<IpAddr>().unwrap(),
        ];

        let selected = select_lease_address(&matches, &prefixes, None, "MAC").unwrap();

        assert_eq!(
            selected,
            "240e:3b2:4e8a:7001::30".parse::<IpAddr>().unwrap()
        );
    }

    #[test]
    fn route_source_prefix_ignores_other_wan_devices() {
        let prefixes = parse_interface_route_source_ipv6_prefixes(
            "default from 2409:8a55:4e26:6980::/60 dev pppoe-wan_cmcc proto static metric 512 pref medium\n\
             default from all dev pppoe-wan proto static metric 512 pref medium\n",
            "pppoe-wan",
        );
        let matches = vec!["2409:8a55:4e26:6980::30".parse::<IpAddr>().unwrap()];

        let err = select_lease_address(&matches, &prefixes, None, "MAC")
            .expect_err("source prefixes from other WAN devices must be ignored");

        assert!(
            err.to_string().contains("WAN/upstream source prefix"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn multi_wan_route_source_prefixes_are_merged() {
        let output =
            "default from 240e:3b2:4e8a:70a0::/60 dev pppoe-wan proto static metric 512 pref medium\n\
             default from 2409:8a55:4e26:6980::/60 dev pppoe-wan_cmcc proto static metric 512 pref medium\n";
        let mut prefixes = parse_interface_route_source_ipv6_prefixes(output, "pppoe-wan");
        for prefix in parse_interface_route_source_ipv6_prefixes(output, "pppoe-wan_cmcc") {
            push_unique_prefix(&mut prefixes, prefix);
        }
        let matches = vec!["2409:8a55:4e26:6980::30".parse::<IpAddr>().unwrap()];

        let selected = select_lease_address(&matches, &prefixes, None, "MAC").unwrap();

        assert_eq!(
            selected,
            "2409:8a55:4e26:6980::30".parse::<IpAddr>().unwrap()
        );
    }

    #[test]
    fn prefix_filter_cannot_replace_wan_source_prefix() {
        let prefixes = parse_interface_route_source_ipv6_prefixes(
            "default from 240e:3b2:4e8a:70a0::/64 dev pppoe-wan proto static metric 512 pref medium\n",
            "pppoe-wan",
        );
        let matches = vec!["2409:8a55:4e26:6980::30".parse::<IpAddr>().unwrap()];

        let err = select_lease_address(&matches, &prefixes, Some("2409:"), "MAC")
            .expect_err("prefix_filter must not bypass WAN source prefix matching");

        assert!(
            err.to_string().contains("WAN/upstream source prefix"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn wan_addr_prefix_is_not_used_for_dhcpv6_validation() {
        let prefixes = parse_interface_route_source_ipv6_prefixes("", "pppoe-wan");
        let matches = vec!["240e:3b2:4e8a:70a0::30".parse::<IpAddr>().unwrap()];

        let err = select_lease_address(&matches, &prefixes, None, "MAC")
            .expect_err("WAN interface address prefixes must not validate LAN host IPv6");

        assert!(
            err.to_string().contains("WAN/upstream source prefix"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn wan_source_prefix_rejects_wrong_prefix_and_non_global_ipv6() {
        let prefixes = parse_interface_route_source_ipv6_prefixes(
            "default from 240e:3b2:4e8a:70a0::/64 dev pppoe-wan proto static metric 512 pref medium\n",
            "pppoe-wan",
        );
        let matches = vec![
            "240e:3b2:4e8a:70a1::30".parse::<IpAddr>().unwrap(),
            "fe80::1".parse::<IpAddr>().unwrap(),
            "fd00::1".parse::<IpAddr>().unwrap(),
            "::1".parse::<IpAddr>().unwrap(),
            "2001:db8::1".parse::<IpAddr>().unwrap(),
        ];

        let err = select_lease_address(&matches, &prefixes, None, "MAC")
            .expect_err("wrong prefix and non-global addresses must be rejected");

        assert!(
            err.to_string().contains("WAN/upstream source prefix"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn wan_source_prefix_applies_prefix_filter_as_narrowing_only() {
        let prefixes = parse_interface_route_source_ipv6_prefixes(
            "default from 240e:3b2:4e8a::/48 dev pppoe-wan proto static metric 512 pref medium\n",
            "pppoe-wan",
        );
        let matches = vec![
            "240e:3b2:4e8a:70a1::30".parse::<IpAddr>().unwrap(),
            "240e:3b2:4e8a:70a0::30".parse::<IpAddr>().unwrap(),
            "2409:8a55:4e26:6980::30".parse::<IpAddr>().unwrap(),
        ];

        let selected =
            select_lease_address(&matches, &prefixes, Some("240e:3b2:4e8a:70a0:"), "DUID").unwrap();

        assert_eq!(
            selected,
            "240e:3b2:4e8a:70a0::30".parse::<IpAddr>().unwrap()
        );
    }

    #[test]
    fn wan_source_prefix_selects_first_matching_candidate_without_prefix_filter() {
        let prefixes = parse_interface_route_source_ipv6_prefixes(
            "default from 240e:3b2:4e8a:70a0::/64 dev pppoe-wan proto static metric 512 pref medium\n",
            "pppoe-wan",
        );
        let matches = vec![
            "240e:3b2:4e8a:70a0::30".parse::<IpAddr>().unwrap(),
            "240e:3b2:4e8a:70a0::31".parse::<IpAddr>().unwrap(),
        ];

        let selected = select_lease_address(&matches, &prefixes, None, "MAC").unwrap();

        assert_eq!(
            selected,
            "240e:3b2:4e8a:70a0::30".parse::<IpAddr>().unwrap()
        );
    }

    #[test]
    fn source_command_output_times_out_slow_commands() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "sleep 2"]);

        let err = command_output_with_timeout(
            &mut command,
            std::time::Duration::from_millis(50),
            "slow source command",
        )
        .expect_err("slow source commands must time out");

        assert!(
            err.to_string().contains("timed out"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn interface_names_accept_multi_select_values() {
        let source = SourceConfig {
            name: "lan_mac".into(),
            kind: SourceKind::Dhcpv6Mac {
                mac: Some("bc-fc-e7-8c-41-cb".into()),
                interface: Some("eth1, wan6,eth2".into()),
                lease_file: None,
                prefix_filter: None,
                hostname_hint: None,
            },
        };

        let names = required_dhcpv6_interfaces(&source, Some("eth1, wan6,eth2"))
            .expect("multi-select interface values must parse");

        assert_eq!(names, vec!["eth1", "wan6", "eth2"]);
    }

    #[test]
    fn dhcpv6_resolution_fails_without_wan_source_prefix() {
        let matches = vec!["240e:3b2:4e8a:70a0::30".parse::<IpAddr>().unwrap()];
        let err = select_lease_address(&matches, &[], None, "DUID")
            .expect_err("no WAN source prefix must fail");

        assert!(
            err.to_string().contains("WAN/upstream source prefix"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn dhcpv6_lease_reader_rejects_pseudo_files() {
        let err = read_dhcpv6_lease_file("/proc/kmsg")
            .expect_err("pseudo files must not be opened as DHCPv6 leases");

        assert!(
            err.to_string().contains("not allowed"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn dhcpv6_lease_reader_accepts_regular_files() {
        let path =
            std::env::temp_dir().join(format!("qddns-source-test-lease-{}", std::process::id()));
        fs::write(&path, "# lease\n").unwrap();

        let content = read_dhcpv6_lease_file(path.to_str().unwrap())
            .expect("regular DHCPv6 lease files must be readable");

        let _ = fs::remove_file(&path);
        assert_eq!(content, "# lease\n");
    }

    #[cfg(unix)]
    #[test]
    fn dhcpv6_lease_reader_accepts_regular_file_symlinks() {
        let lease_path = std::env::temp_dir().join(format!(
            "qddns-source-test-lease-real-{}",
            std::process::id()
        ));
        let link_path = std::env::temp_dir().join(format!(
            "qddns-source-test-lease-real-link-{}",
            std::process::id()
        ));
        let _ = fs::remove_file(&lease_path);
        let _ = fs::remove_file(&link_path);
        fs::write(&lease_path, "# lease\n").unwrap();
        std::os::unix::fs::symlink(&lease_path, &link_path).unwrap();

        let content = read_dhcpv6_lease_file(link_path.to_str().unwrap())
            .expect("regular DHCPv6 lease file symlinks must be readable");

        let _ = fs::remove_file(&link_path);
        let _ = fs::remove_file(&lease_path);
        assert_eq!(content, "# lease\n");
    }

    #[cfg(unix)]
    #[test]
    fn dhcpv6_lease_reader_rejects_pseudo_file_symlinks() {
        let path = std::env::temp_dir().join(format!(
            "qddns-source-test-lease-link-{}",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);
        std::os::unix::fs::symlink("/proc/kmsg", &path).unwrap();

        let err = read_dhcpv6_lease_file(path.to_str().unwrap())
            .expect_err("pseudo file symlinks must not be opened as DHCPv6 leases");

        let _ = fs::remove_file(&path);
        assert!(
            err.to_string().contains("not allowed"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn dhcpv6_lease_reader_rejects_non_regular_paths() {
        let path =
            std::env::temp_dir().join(format!("qddns-source-test-dir-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();

        let err = read_dhcpv6_lease_file(path.to_str().unwrap())
            .expect_err("directories must not be read as DHCPv6 leases");

        let _ = fs::remove_dir_all(&path);
        assert!(
            err.to_string().contains("regular file"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn dhcpv6_lease_reader_rejects_oversized_files() {
        let path =
            std::env::temp_dir().join(format!("qddns-source-test-lease-{}", std::process::id()));
        fs::write(&path, "x".repeat((DHCPV6_LEASE_MAX_BYTES + 1) as usize)).unwrap();

        let err = read_dhcpv6_lease_file(path.to_str().unwrap())
            .expect_err("oversized leases must be rejected");

        let _ = fs::remove_file(&path);
        assert!(
            err.to_string().contains("exceeds"),
            "unexpected error: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn dhcpv6_mac_rejects_unsafe_explicit_lease_file_before_ndp_fallback() {
        let source = SourceConfig {
            name: "probe".into(),
            kind: SourceKind::Dhcpv6Mac {
                mac: Some("1a:26:b5:c1:c3:d0".into()),
                interface: Some("eth1".into()),
                lease_file: Some("/proc/kmsg".into()),
                prefix_filter: None,
                hostname_hint: None,
            },
        };
        let result = resolve_source(&source);

        let err = result.expect_err("unsafe explicit lease_file must not reach interface probing");
        assert!(
            err.to_string().contains("not allowed"),
            "unexpected error: {err}"
        );
    }
}
