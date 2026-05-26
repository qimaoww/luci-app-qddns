use std::collections::BTreeMap;
use std::fs;
use std::net::IpAddr;
use std::process::Command;

use crate::config::{AddressFamily, SourceConfig, SourceKind};
use crate::error::{Error, Result};
use crate::http::{HttpClient, HttpRequest, RetryPolicy};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceResolution {
    pub address: IpAddr,
    pub family: String,
    pub detail: String,
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
            lease_file,
            prefix_filter,
            ..
        } => resolve_dhcpv6_duid(
            source,
            duid.as_deref(),
            iaid.as_deref(),
            lease_file.as_deref(),
            prefix_filter.as_deref(),
        ),
        SourceKind::Dhcpv6Mac {
            mac,
            lease_file,
            prefix_filter,
            ..
        } => resolve_dhcpv6_mac(
            source,
            mac.as_deref(),
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
    lease_file: Option<&str>,
    prefix_filter: Option<&str>,
) -> Result<SourceResolution> {
    let duid = duid.ok_or_else(|| Error::new(format!("source '{}' missing duid", source.name)))?;
    let iaid = iaid.ok_or_else(|| Error::new(format!("source '{}' missing iaid", source.name)))?;
    let lease_file = lease_file.unwrap_or("/tmp/odhcpd.leases");

    let content = fs::read_to_string(lease_file)
        .map_err(|err| Error::new(format!("failed to read lease file '{lease_file}': {err}")))?;

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

    let selected = select_lease_address(&matches, prefix_filter, &format!("DUID '{duid}'"))?;

    Ok(SourceResolution {
        address: selected,
        family: "ipv6".into(),
        detail: format!("resolved from {lease_file}"),
    })
}

fn resolve_dhcpv6_mac(
    source: &SourceConfig,
    mac: Option<&str>,
    lease_file: Option<&str>,
    prefix_filter: Option<&str>,
) -> Result<SourceResolution> {
    let mac = mac.ok_or_else(|| Error::new(format!("source '{}' missing mac", source.name)))?;
    let normalized_mac = normalize_mac(mac)?;
    let lease_file = lease_file.unwrap_or("/tmp/odhcpd.leases");

    let mut matches = Vec::<IpAddr>::new();
    collect_host_hint_ipv6_candidates(&normalized_mac, &mut matches);

    let content = fs::read_to_string(lease_file).unwrap_or_default();
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

    collect_ndp_ipv6_candidates(&normalized_mac, &mut matches);

    if matches.is_empty() {
        return Err(Error::new(format!(
            "no public IPv6 address found for MAC '{}'",
            format_mac(&normalized_mac)
        )));
    }

    let selected = select_lease_address(
        &matches,
        prefix_filter,
        &format!("MAC '{}'", format_mac(&normalized_mac)),
    )?;

    Ok(SourceResolution {
        address: selected,
        family: "ipv6".into(),
        detail: format!("resolved by MAC from LAN host tables and {lease_file}"),
    })
}

fn collect_public_ipv6_candidates(fields: &[&str], matches: &mut Vec<IpAddr>) {
    for candidate in fields.iter().skip(8) {
        let raw = candidate.split('/').next().unwrap_or("");
        push_public_ipv6(raw, matches);
    }
}

fn collect_ndp_ipv6_candidates(normalized_mac: &str, matches: &mut Vec<IpAddr>) {
    let Ok(output) = Command::new("ip").args(["-6", "neigh", "show"]).output() else {
        return;
    };
    if !output.status.success() {
        return;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    collect_ndp_ipv6_candidates_from_output(&stdout, normalized_mac, matches);
}

fn collect_host_hint_ipv6_candidates(normalized_mac: &str, matches: &mut Vec<IpAddr>) {
    let Ok(output) = Command::new("ubus")
        .args(["call", "luci-rpc", "getHostHints"])
        .output()
    else {
        return;
    };
    if !output.status.success() {
        return;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    collect_host_hint_ipv6_candidates_from_json(&stdout, normalized_mac, matches);
}

fn collect_host_hint_ipv6_candidates_from_json(
    input: &str,
    normalized_mac: &str,
    matches: &mut Vec<IpAddr>,
) {
    let Ok(value) = serde_json::from_str::<Value>(input) else {
        return;
    };
    let Some(hosts) = value.as_object() else {
        return;
    };

    for (mac, host) in hosts {
        if normalize_mac(mac).ok().as_deref() != Some(normalized_mac) {
            continue;
        }

        let Some(addresses) = host.get("ip6addrs").and_then(Value::as_array) else {
            continue;
        };
        for address in addresses.iter().filter_map(Value::as_str) {
            push_public_ipv6(address, matches);
        }
    }
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

fn select_lease_address(
    matches: &[IpAddr],
    prefix_filter: Option<&str>,
    subject: &str,
) -> Result<IpAddr> {
    if let Some(prefix) = prefix_filter {
        return matches
            .iter()
            .find(|addr| addr.to_string().starts_with(prefix))
            .copied()
            .ok_or_else(|| {
                Error::new(format!(
                    "no IPv6 lease matched prefix '{prefix}' for {subject}"
                ))
            });
    }

    if matches.len() == 1 {
        return Ok(matches[0]);
    }

    Err(Error::new(format!(
        "multiple IPv6 addresses found for {subject}, prefix filter required"
    )))
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
    let output = Command::new(script)
        .output()
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
    let iface = interface
        .ok_or_else(|| Error::new(format!("source '{}' missing interface", source.name)))?;
    validate_interface_name(iface)?;
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

    let output = Command::new("ip")
        .args(["addr", "show", "dev", iface])
        .output()
        .map_err(|err| Error::new(format!("failed to inspect interface '{iface}': {err}")))?;
    if !output.status.success() {
        return Err(Error::new(format!(
            "unable to inspect interface '{}'",
            iface
        )));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_interface_address(&stdout, family)
        .ok_or_else(|| Error::new(format!("interface '{}' has no address", iface)))
        .and_then(|address| parse_address_output(&address.to_string(), "interface"))
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
    fn host_hints_collects_unique_public_ipv6_for_mac() {
        let mut matches = Vec::new();

        collect_host_hint_ipv6_candidates_from_json(
            r#"{
                "BC:FC:E7:8C:41:CB": {
                    "ip6addrs": [
                        "fe80::bd:301:a658:3234",
                        "fd00::30",
                        "2001:db8::30",
                        "240e:3b2:4e8a:70a0::30",
                        "240e:3b2:4e8a:70a0::30",
                        "2409:8a55:4e26:6980::30"
                    ]
                },
                "10:7C:61:B2:07:01": {
                    "ip6addrs": [ "240e:3b2:4e8a:70a0::205" ]
                }
            }"#,
            "bcfce78c41cb",
            &mut matches,
        );

        assert_eq!(
            matches,
            vec![
                "240e:3b2:4e8a:70a0::30".parse::<IpAddr>().unwrap(),
                "2409:8a55:4e26:6980::30".parse::<IpAddr>().unwrap(),
            ]
        );
    }
}
