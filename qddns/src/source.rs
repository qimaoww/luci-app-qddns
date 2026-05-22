use std::fs;
use std::net::IpAddr;
use std::process::Command;

use crate::config::SourceConfig;
use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceResolution {
    pub address: IpAddr,
    pub family: String,
    pub detail: String,
}

pub fn resolve_source(source: &SourceConfig) -> Result<SourceResolution> {
    match source.source_type.as_str() {
        "local_addr" => resolve_local_addr(source),
        "command" => resolve_command(source),
        "script" => resolve_script(source),
        "public_probe" => resolve_public_probe(source),
        "interface" => resolve_interface(source),
        "dhcpv6_duid" => resolve_dhcpv6_duid(source),
        other => Err(Error::new(format!(
            "source '{}' has unsupported type '{other}'",
            source.name
        ))),
    }
}

fn resolve_local_addr(source: &SourceConfig) -> Result<SourceResolution> {
    let address = source
        .address
        .as_deref()
        .ok_or_else(|| Error::new(format!("source '{}' missing address", source.name)))?
        .parse::<IpAddr>()
        .map_err(|err| Error::new(format!("invalid address: {err}")))?;

    Ok(SourceResolution {
        family: if address.is_ipv4() { "ipv4" } else { "ipv6" }.into(),
        detail: "configured local address".into(),
        address,
    })
}

fn resolve_dhcpv6_duid(source: &SourceConfig) -> Result<SourceResolution> {
    let duid = source
        .duid
        .as_deref()
        .ok_or_else(|| Error::new(format!("source '{}' missing duid", source.name)))?;
    let iaid = source
        .iaid
        .as_deref()
        .ok_or_else(|| Error::new(format!("source '{}' missing iaid", source.name)))?;
    let lease_file = source
        .lease_file
        .as_deref()
        .unwrap_or("/tmp/odhcpd.leases");

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

        for candidate in fields.iter().skip(8) {
            let raw = candidate.split('/').next().unwrap_or("");
            if !raw.starts_with('2') && !raw.starts_with('3') {
                continue;
            }
            if let Ok(ip) = raw.parse::<IpAddr>() {
                if ip.is_ipv6() {
                    matches.push(ip);
                }
            }
        }
    }

    if matches.is_empty() {
        return Err(Error::new(format!(
            "no IPv6 lease found for DUID '{duid}' and IAID '{iaid}'"
        )));
    }

    let selected = if let Some(prefix) = source.prefix_filter.as_deref() {
        matches
            .iter()
            .find(|addr| addr.to_string().starts_with(prefix))
            .copied()
            .ok_or_else(|| {
                Error::new(format!(
                    "no IPv6 lease matched prefix '{prefix}' for DUID '{duid}'"
                ))
            })?
    } else if matches.len() == 1 {
        matches[0]
    } else {
        return Err(Error::new(format!(
            "multiple IPv6 addresses found for DUID '{duid}', prefix filter required"
        )));
    };

    Ok(SourceResolution {
        address: selected,
        family: "ipv6".into(),
        detail: format!("resolved from {lease_file}"),
    })
}

fn resolve_command(source: &SourceConfig) -> Result<SourceResolution> {
    let command = source
        .command
        .as_deref()
        .ok_or_else(|| Error::new(format!("source '{}' missing command", source.name)))?;
    let output = Command::new("sh")
        .args(["-c", command])
        .output()
        .map_err(|err| Error::new(format!("failed to execute command: {err}")))?;
    if !output.status.success() {
        return Err(Error::new(format!(
            "command source '{}' exited with status {}",
            source.name, output.status
        )));
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    parse_address_output(&stdout, "shell command")
}

fn resolve_script(source: &SourceConfig) -> Result<SourceResolution> {
    let script = source
        .script
        .as_deref()
        .ok_or_else(|| Error::new(format!("source '{}' missing script", source.name)))?;
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

fn resolve_public_probe(source: &SourceConfig) -> Result<SourceResolution> {
    let probe_url = source
        .probe_url
        .as_deref()
        .ok_or_else(|| Error::new(format!("source '{}' missing probe_url", source.name)))?;

    let body = if let Some(path) = probe_url.strip_prefix("file://") {
        fs::read_to_string(path)
            .map_err(|err| Error::new(format!("failed to read probe file '{path}': {err}")))?
    } else if probe_url.starts_with("http://") || probe_url.starts_with("https://") {
        let output = Command::new("curl")
            .args(["-fsSL", probe_url])
            .output()
            .map_err(|err| Error::new(format!("failed to execute public probe curl: {err}")))?;
        if !output.status.success() {
            return Err(Error::new(format!(
                "public probe '{}' exited with status {}",
                source.name, output.status
            )));
        }
        String::from_utf8_lossy(&output.stdout).to_string()
    } else {
        return Err(Error::new(format!(
            "source '{}' has unsupported probe URL scheme",
            source.name
        )));
    };

    let candidate = find_ip_in_text(&body)
        .ok_or_else(|| Error::new(format!("no IP address found in probe response for '{}'", source.name)))?;
    parse_address_output(candidate, "public probe")
}

fn resolve_interface(source: &SourceConfig) -> Result<SourceResolution> {
    let iface = source
        .interface
        .as_deref()
        .ok_or_else(|| Error::new(format!("source '{}' missing interface", source.name)))?;
    if iface == "lo" {
        let address = match source.family.as_deref() {
            Some("ipv6") => "::1".parse::<IpAddr>().unwrap(),
            _ => "127.0.0.1".parse::<IpAddr>().unwrap(),
        };
        return Ok(SourceResolution {
            family: if address.is_ipv4() { "ipv4" } else { "ipv6" }.into(),
            detail: format!("loopback fallback for interface {iface}"),
            address,
        });
    }

    let output = Command::new("sh")
        .args([
            "-c",
            &format!("ip addr show dev {} 2>/dev/null | grep -Eo 'inet6? [^ /]+' | awk '{{print $2}}' | head -n1", iface),
        ])
        .output()
        .map_err(|err| Error::new(format!("failed to inspect interface '{iface}': {err}")))?;
    if !output.status.success() {
        return Err(Error::new(format!(
            "unable to inspect interface '{}'",
            iface
        )));
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        return Err(Error::new(format!("interface '{}' has no address", iface)));
    }
    parse_address_output(&stdout, "interface")
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
    for token in text.split(|c: char| c.is_whitespace() || [',', ';', '[', ']', '(', ')'].contains(&c)) {
        if token.parse::<IpAddr>().is_ok() {
            return Some(token);
        }
    }
    None
}
