use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Config {
    pub main: MainConfig,
    pub sources: BTreeMap<String, SourceConfig>,
    pub providers: BTreeMap<String, ProviderConfig>,
    pub rules: BTreeMap<String, RuleConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MainConfig {
    pub enabled: bool,
    pub log_dir: String,
    pub state_dir: String,
    pub listen: String,
    pub poll_interval: u64,
    pub timeout: u64,
    pub log_level: String,
}

impl Default for MainConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            log_dir: "/var/log/qddns".into(),
            state_dir: "/var/run/qddns".into(),
            listen: "127.0.0.1:53530".into(),
            poll_interval: 60,
            timeout: 15,
            log_level: "info".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceConfig {
    pub name: String,
    pub source_type: String,
    pub family: Option<String>,
    pub interface: Option<String>,
    pub address: Option<String>,
    pub probe_url: Option<String>,
    pub script: Option<String>,
    pub command: Option<String>,
    pub duid: Option<String>,
    pub iaid: Option<String>,
    pub lease_file: Option<String>,
    pub prefix_filter: Option<String>,
    pub hostname_hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderConfig {
    pub name: String,
    pub provider_type: String,
    pub api_token: Option<String>,
    pub secret_id: Option<String>,
    pub secret_key: Option<String>,
    pub access_key_id: Option<String>,
    pub access_key_secret: Option<String>,
    pub url: Option<String>,
    pub method: Option<String>,
    pub headers_json: Option<String>,
    pub body_template: Option<String>,
    pub lookup_url: Option<String>,
    pub lookup_method: Option<String>,
    pub lookup_headers_json: Option<String>,
    pub lookup_json_pointer: Option<String>,
    pub success_contains: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleConfig {
    pub name: String,
    pub enabled: bool,
    pub provider: String,
    pub source: String,
    pub record_type: String,
    pub zone: String,
    pub record_name: String,
    pub ttl: u32,
    pub proxied: bool,
    pub check_interval: u64,
    pub force_interval: u64,
    pub retry_count: u32,
    pub retry_backoff: u64,
}

impl Config {
    pub fn load_from_path(path: &Path) -> Result<Self> {
        if path == Path::new("/dev/null") {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(path)?;
        Self::parse_uci(&content)
    }

    pub fn validate(&self) -> Result<()> {
        for (rule_id, rule) in &self.rules {
            let source = self.sources.get(&rule.source).ok_or_else(|| {
                Error::new(format!("rule '{rule_id}' references missing source '{}'", rule.source))
            })?;
            let _provider = self.providers.get(&rule.provider).ok_or_else(|| {
                Error::new(format!(
                    "rule '{rule_id}' references missing provider '{}'",
                    rule.provider
                ))
            })?;

            match rule.record_type.as_str() {
                "A" | "AAAA" => {}
                other => {
                    return Err(Error::new(format!(
                        "rule '{rule_id}' has unsupported record_type '{other}'"
                    )))
                }
            }

            if rule.record_type == "AAAA" {
                if source.family.as_deref() == Some("ipv4") {
                    return Err(Error::new(format!(
                        "rule '{rule_id}' with AAAA cannot use IPv4 source '{}'",
                        source.name
                    )));
                }
                if let Some(address) = source.address.as_deref() {
                    if address.parse::<std::net::Ipv4Addr>().is_ok() {
                        return Err(Error::new(format!(
                            "rule '{rule_id}' with AAAA cannot use IPv4 address '{address}'"
                        )));
                    }
                }
            }

            if rule.record_type == "A" {
                if source.family.as_deref() == Some("ipv6") {
                    return Err(Error::new(format!(
                        "rule '{rule_id}' with A cannot use IPv6 source '{}'",
                        source.name
                    )));
                }
                if let Some(address) = source.address.as_deref() {
                    if address.parse::<std::net::Ipv6Addr>().is_ok() {
                        return Err(Error::new(format!(
                            "rule '{rule_id}' with A cannot use IPv6 address '{address}'"
                        )));
                    }
                }
            }
        }

        Ok(())
    }

    pub fn parse_uci(input: &str) -> Result<Self> {
        #[derive(Debug)]
        struct Section {
            kind: String,
            name: String,
            options: BTreeMap<String, String>,
        }

        let mut sections = Vec::<Section>::new();
        let mut current: Option<Section> = None;

        for raw_line in input.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if line.starts_with("config ") {
                if let Some(section) = current.take() {
                    sections.push(section);
                }
                let parts = split_uci_tokens(line);
                if parts.len() < 3 {
                    return Err(Error::new(format!("invalid config line: {line}")));
                }
                current = Some(Section {
                    kind: parts[1].clone(),
                    name: parts[2].clone(),
                    options: BTreeMap::new(),
                });
                continue;
            }

            if line.starts_with("option ") {
                let parts = split_uci_tokens(line);
                if parts.len() < 3 {
                    return Err(Error::new(format!("invalid option line: {line}")));
                }
                let section = current
                    .as_mut()
                    .ok_or_else(|| Error::new(format!("option outside config section: {line}")))?;
                section.options.insert(parts[1].clone(), parts[2].clone());
            }
        }

        if let Some(section) = current.take() {
            sections.push(section);
        }

        let mut config = Config::default();
        for section in sections {
            match section.kind.as_str() {
                "qddns" => {
                    config.main = MainConfig {
                        enabled: parse_bool(section.options.get("enabled").map(String::as_str), true),
                        log_dir: get_string(&section.options, "log_dir")
                            .unwrap_or_else(|| config.main.log_dir.clone()),
                        state_dir: get_string(&section.options, "state_dir")
                            .unwrap_or_else(|| config.main.state_dir.clone()),
                        listen: get_string(&section.options, "listen")
                            .unwrap_or_else(|| config.main.listen.clone()),
                        poll_interval: parse_u64(
                            section.options.get("poll_interval").map(String::as_str),
                            config.main.poll_interval,
                        ),
                        timeout: parse_u64(
                            section.options.get("timeout").map(String::as_str),
                            config.main.timeout,
                        ),
                        log_level: get_string(&section.options, "log_level")
                            .unwrap_or_else(|| config.main.log_level.clone()),
                    };
                }
                "source" => {
                    config.sources.insert(
                        section.name.clone(),
                        SourceConfig {
                            name: section.name.clone(),
                            source_type: get_required_string(&section.options, "type", &section.name)?,
                            family: get_string(&section.options, "family"),
                            interface: get_string(&section.options, "interface"),
                            address: get_string(&section.options, "address"),
                            probe_url: get_string(&section.options, "probe_url")
                                .or_else(|| get_string(&section.options, "url")),
                            script: get_string(&section.options, "script"),
                            command: get_string(&section.options, "command"),
                            duid: get_string(&section.options, "duid"),
                            iaid: get_string(&section.options, "iaid"),
                            lease_file: get_string(&section.options, "lease_file"),
                            prefix_filter: get_string(&section.options, "prefix_filter"),
                            hostname_hint: get_string(&section.options, "hostname_hint"),
                        },
                    );
                }
                "provider" => {
                    config.providers.insert(
                        section.name.clone(),
                        ProviderConfig {
                            name: section.name.clone(),
                            provider_type: get_required_string(&section.options, "type", &section.name)?,
                            api_token: get_string(&section.options, "api_token"),
                            secret_id: get_string(&section.options, "secret_id"),
                            secret_key: get_string(&section.options, "secret_key"),
                            access_key_id: get_string(&section.options, "access_key_id"),
                            access_key_secret: get_string(&section.options, "access_key_secret"),
                            url: get_string(&section.options, "url"),
                            method: get_string(&section.options, "method"),
                            headers_json: get_string(&section.options, "headers_json"),
                            body_template: get_string(&section.options, "body_template"),
                            lookup_url: get_string(&section.options, "lookup_url"),
                            lookup_method: get_string(&section.options, "lookup_method"),
                            lookup_headers_json: get_string(&section.options, "lookup_headers_json"),
                            lookup_json_pointer: get_string(&section.options, "lookup_json_pointer"),
                            success_contains: get_string(&section.options, "success_contains"),
                        },
                    );
                }
                "rule" => {
                    config.rules.insert(
                        section.name.clone(),
                        RuleConfig {
                            name: section.name.clone(),
                            enabled: parse_bool(section.options.get("enabled").map(String::as_str), true),
                            provider: get_required_string(&section.options, "provider", &section.name)?,
                            source: get_required_string(&section.options, "source", &section.name)?,
                            record_type: get_required_string(
                                &section.options,
                                "record_type",
                                &section.name,
                            )?,
                            zone: get_required_string(&section.options, "zone", &section.name)?,
                            record_name: get_required_string(
                                &section.options,
                                "record_name",
                                &section.name,
                            )?,
                            ttl: parse_u32(section.options.get("ttl").map(String::as_str), 300),
                            proxied: parse_bool(section.options.get("proxied").map(String::as_str), false),
                            check_interval: parse_u64(
                                section.options.get("check_interval").map(String::as_str),
                                60,
                            ),
                            force_interval: parse_u64(
                                section.options.get("force_interval").map(String::as_str),
                                3600,
                            ),
                            retry_count: parse_u32(
                                section.options.get("retry_count").map(String::as_str),
                                3,
                            ),
                            retry_backoff: parse_u64(
                                section.options.get("retry_backoff").map(String::as_str),
                                30,
                            ),
                        },
                    );
                }
                other => return Err(Error::new(format!("unsupported section type '{other}'"))),
            }
        }

        Ok(config)
    }
}

fn split_uci_tokens(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;

    for ch in line.chars() {
        match quote {
            Some(q) if ch == q => quote = None,
            Some(_) => current.push(ch),
            None if ch == '\'' || ch == '"' => quote = Some(ch),
            None if ch.is_whitespace() => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            None => current.push(ch),
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

fn get_string(map: &BTreeMap<String, String>, key: &str) -> Option<String> {
    map.get(key).cloned()
}

fn get_required_string(
    map: &BTreeMap<String, String>,
    key: &str,
    section: &str,
) -> Result<String> {
    map.get(key)
        .cloned()
        .ok_or_else(|| Error::new(format!("section '{section}' missing required option '{key}'")))
}

fn parse_bool(value: Option<&str>, default: bool) -> bool {
    match value {
        Some("1") | Some("true") | Some("yes") | Some("on") => true,
        Some("0") | Some("false") | Some("no") | Some("off") => false,
        Some(_) | None => default,
    }
}

fn parse_u64(value: Option<&str>, default: u64) -> u64 {
    value.and_then(|v| v.parse::<u64>().ok()).unwrap_or(default)
}

fn parse_u32(value: Option<&str>, default: u32) -> u32 {
    value.and_then(|v| v.parse::<u32>().ok()).unwrap_or(default)
}
