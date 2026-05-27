use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::error::{Error, Result};

pub type ConfigText = String;

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
    pub name: ConfigText,
    pub kind: SourceKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceKind {
    LocalAddr {
        family: Option<AddressFamily>,
        address: Option<ConfigText>,
    },
    Interface {
        family: Option<AddressFamily>,
        interface: Option<ConfigText>,
    },
    PublicProbe {
        family: Option<AddressFamily>,
        probe_url: Option<ConfigText>,
    },
    Script {
        family: Option<AddressFamily>,
        script: Option<ConfigText>,
    },
    Dhcpv6Duid {
        duid: Option<ConfigText>,
        iaid: Option<ConfigText>,
        interface: Option<ConfigText>,
        lease_file: Option<ConfigText>,
        prefix_filter: Option<ConfigText>,
        hostname_hint: Option<ConfigText>,
    },
    Dhcpv6Mac {
        mac: Option<ConfigText>,
        interface: Option<ConfigText>,
        lease_file: Option<ConfigText>,
        prefix_filter: Option<ConfigText>,
        hostname_hint: Option<ConfigText>,
    },
}

impl SourceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SourceKind::LocalAddr { .. } => "local_addr",
            SourceKind::Interface { .. } => "interface",
            SourceKind::PublicProbe { .. } => "public_probe",
            SourceKind::Script { .. } => "script",
            SourceKind::Dhcpv6Duid { .. } => "dhcpv6_duid",
            SourceKind::Dhcpv6Mac { .. } => "dhcpv6_mac",
        }
    }

    pub fn family(&self) -> Option<AddressFamily> {
        match self {
            SourceKind::LocalAddr { family, .. }
            | SourceKind::Interface { family, .. }
            | SourceKind::PublicProbe { family, .. }
            | SourceKind::Script { family, .. } => *family,
            SourceKind::Dhcpv6Duid { .. } | SourceKind::Dhcpv6Mac { .. } => {
                Some(AddressFamily::Ipv6)
            }
        }
    }

    pub fn address(&self) -> Option<&str> {
        match self {
            SourceKind::LocalAddr { address, .. } => address.as_deref(),
            _ => None,
        }
    }
}

impl SourceConfig {
    pub fn source_type(&self) -> &'static str {
        self.kind.as_str()
    }

    pub fn family(&self) -> Option<AddressFamily> {
        self.kind.family()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderConfig {
    pub name: ConfigText,
    pub kind: ProviderKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderKind {
    Cloudflare {
        api_token: Option<ConfigText>,
    },
    DnsPod {
        secret_id: Option<ConfigText>,
        secret_key: Option<ConfigText>,
    },
    Aliyun {
        access_key_id: Option<ConfigText>,
        access_key_secret: Option<ConfigText>,
    },
    CustomHttp(CustomHttpConfig),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CustomHttpConfig {
    pub url: Option<ConfigText>,
    pub method: Option<ConfigText>,
    pub headers_json: Option<ConfigText>,
    pub body_template: Option<ConfigText>,
    pub lookup_url: Option<ConfigText>,
    pub lookup_method: Option<ConfigText>,
    pub lookup_headers_json: Option<ConfigText>,
    pub lookup_json_pointer: Option<ConfigText>,
    pub success_contains: Option<ConfigText>,
}

impl ProviderKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProviderKind::Cloudflare { .. } => "cloudflare",
            ProviderKind::DnsPod { .. } => "dnspod",
            ProviderKind::Aliyun { .. } => "aliyun",
            ProviderKind::CustomHttp(_) => "custom_http",
        }
    }
}

impl ProviderConfig {
    pub fn provider_type(&self) -> &'static str {
        self.kind.as_str()
    }

    pub fn custom_http(&self) -> Option<&CustomHttpConfig> {
        match &self.kind {
            ProviderKind::CustomHttp(config) => Some(config),
            _ => None,
        }
    }

    pub fn cloudflare_api_token(&self) -> Option<&str> {
        match &self.kind {
            ProviderKind::Cloudflare { api_token } => api_token.as_deref(),
            _ => None,
        }
    }

    pub fn dnspod_credentials(&self) -> (Option<&str>, Option<&str>) {
        match &self.kind {
            ProviderKind::DnsPod {
                secret_id,
                secret_key,
            } => (secret_id.as_deref(), secret_key.as_deref()),
            _ => (None, None),
        }
    }

    pub fn aliyun_credentials(&self) -> (Option<&str>, Option<&str>) {
        match &self.kind {
            ProviderKind::Aliyun {
                access_key_id,
                access_key_secret,
            } => (access_key_id.as_deref(), access_key_secret.as_deref()),
            _ => (None, None),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleConfig {
    pub name: ConfigText,
    pub enabled: bool,
    pub provider: ConfigText,
    pub source: ConfigText,
    pub record_type: RecordType,
    pub zone: ConfigText,
    pub record_name: ConfigText,
    pub ttl: u32,
    pub proxied: bool,
    pub check_interval: u64,
    pub force_interval: u64,
    pub retry_count: u32,
    pub retry_backoff: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordType {
    A,
    Aaaa,
}

impl RecordType {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "A" => Ok(RecordType::A),
            "AAAA" => Ok(RecordType::Aaaa),
            other => Err(Error::new(format!("unsupported record_type '{other}'"))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            RecordType::A => "A",
            RecordType::Aaaa => "AAAA",
        }
    }
}

impl std::fmt::Display for RecordType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl PartialEq<&str> for RecordType {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl From<&str> for RecordType {
    fn from(value: &str) -> Self {
        RecordType::parse(value).expect("valid record type")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressFamily {
    Ipv4,
    Ipv6,
}

impl AddressFamily {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "ipv4" => Ok(AddressFamily::Ipv4),
            "ipv6" => Ok(AddressFamily::Ipv6),
            other => Err(Error::new(format!("unsupported family '{other}'"))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            AddressFamily::Ipv4 => "ipv4",
            AddressFamily::Ipv6 => "ipv6",
        }
    }
}

impl std::fmt::Display for AddressFamily {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
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
        for (source_id, source) in &self.sources {
            match &source.kind {
                SourceKind::Dhcpv6Duid { interface, .. }
                | SourceKind::Dhcpv6Mac { interface, .. } => {
                    if interface
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .is_none()
                    {
                        return Err(Error::new(format!(
                            "source.{source_id}.interface missing required option"
                        )));
                    }
                }
                _ => {}
            }
        }

        for (rule_id, rule) in &self.rules {
            let source = self.sources.get(&rule.source).ok_or_else(|| {
                Error::new(format!(
                    "rule '{rule_id}' references missing source '{}'",
                    rule.source
                ))
            })?;
            let _provider = self.providers.get(&rule.provider).ok_or_else(|| {
                Error::new(format!(
                    "rule '{rule_id}' references missing provider '{}'",
                    rule.provider
                ))
            })?;

            if rule.record_type == RecordType::Aaaa {
                if source.family() == Some(AddressFamily::Ipv4) {
                    return Err(Error::new(format!(
                        "rule '{rule_id}' with AAAA cannot use IPv4 source '{}'",
                        source.name
                    )));
                }
                if let Some(address) = source.kind.address() {
                    if address.parse::<std::net::Ipv4Addr>().is_ok() {
                        return Err(Error::new(format!(
                            "rule '{rule_id}' with AAAA cannot use IPv4 address '{address}'"
                        )));
                    }
                }
            }

            if rule.record_type == RecordType::A {
                if source.family() == Some(AddressFamily::Ipv6) {
                    return Err(Error::new(format!(
                        "rule '{rule_id}' with A cannot use IPv6 source '{}'",
                        source.name
                    )));
                }
                if let Some(address) = source.kind.address() {
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
                continue;
            }

            if line.starts_with("list ") {
                let parts = split_uci_tokens(line);
                if parts.len() < 3 {
                    return Err(Error::new(format!("invalid list line: {line}")));
                }
                let section = current
                    .as_mut()
                    .ok_or_else(|| Error::new(format!("list outside config section: {line}")))?;
                section
                    .options
                    .entry(parts[1].clone())
                    .and_modify(|value| {
                        if !value.is_empty() {
                            value.push(',');
                        }
                        value.push_str(&parts[2]);
                    })
                    .or_insert_with(|| parts[2].clone());
            }
        }

        if let Some(section) = current.take() {
            sections.push(section);
        }

        let mut config = Config::default();
        for section in sections {
            match section.kind.as_str() {
                "qddns" => {
                    reject_unknown_options(
                        &section.kind,
                        &section.name,
                        &section.options,
                        MAIN_OPTIONS,
                    )?;
                    config.main = MainConfig {
                        enabled: parse_bool(
                            &field_path(&section.kind, &section.name, "enabled"),
                            section.options.get("enabled").map(String::as_str),
                            true,
                        )?,
                        log_dir: get_string(&section.options, "log_dir")
                            .unwrap_or_else(|| config.main.log_dir.clone()),
                        state_dir: get_string(&section.options, "state_dir")
                            .unwrap_or_else(|| config.main.state_dir.clone()),
                        listen: get_string(&section.options, "listen")
                            .unwrap_or_else(|| config.main.listen.clone()),
                        poll_interval: parse_u64_range(
                            &field_path(&section.kind, &section.name, "poll_interval"),
                            section.options.get("poll_interval").map(String::as_str),
                            config.main.poll_interval,
                            1,
                            u64::MAX,
                        )?,
                        timeout: parse_u64_range(
                            &field_path(&section.kind, &section.name, "timeout"),
                            section.options.get("timeout").map(String::as_str),
                            config.main.timeout,
                            1,
                            30,
                        )?,
                        log_level: get_string(&section.options, "log_level")
                            .unwrap_or_else(|| config.main.log_level.clone()),
                    };
                }
                "source" => {
                    reject_unknown_options(
                        &section.kind,
                        &section.name,
                        &section.options,
                        SOURCE_OPTIONS,
                    )?;
                    let source_type = get_required_string(&section.options, "type", &section.name)?;
                    config.sources.insert(
                        section.name.clone(),
                        SourceConfig {
                            name: section.name.clone(),
                            kind: parse_source_kind(&section.name, &source_type, &section.options)?,
                        },
                    );
                }
                "provider" => {
                    reject_unknown_options(
                        &section.kind,
                        &section.name,
                        &section.options,
                        PROVIDER_OPTIONS,
                    )?;
                    let provider_type =
                        get_required_string(&section.options, "type", &section.name)?;
                    config.providers.insert(
                        section.name.clone(),
                        ProviderConfig {
                            name: section.name.clone(),
                            kind: parse_provider_kind(
                                &section.name,
                                &provider_type,
                                &section.options,
                            )?,
                        },
                    );
                }
                "rule" => {
                    reject_unknown_options(
                        &section.kind,
                        &section.name,
                        &section.options,
                        RULE_OPTIONS,
                    )?;
                    config.rules.insert(
                        section.name.clone(),
                        RuleConfig {
                            name: section.name.clone(),
                            enabled: parse_bool(
                                &field_path(&section.kind, &section.name, "enabled"),
                                section.options.get("enabled").map(String::as_str),
                                true,
                            )?,
                            provider: get_required_string(
                                &section.options,
                                "provider",
                                &section.name,
                            )?,
                            source: get_required_string(&section.options, "source", &section.name)?,
                            record_type: RecordType::parse(&get_required_string(
                                &section.options,
                                "record_type",
                                &section.name,
                            )?)?,
                            zone: get_required_string(&section.options, "zone", &section.name)?,
                            record_name: get_required_string(
                                &section.options,
                                "record_name",
                                &section.name,
                            )?,
                            ttl: parse_u32_range(
                                &field_path(&section.kind, &section.name, "ttl"),
                                section.options.get("ttl").map(String::as_str),
                                300,
                                1,
                                86400,
                            )?,
                            proxied: parse_bool(
                                &field_path(&section.kind, &section.name, "proxied"),
                                section.options.get("proxied").map(String::as_str),
                                false,
                            )?,
                            check_interval: parse_u64_range(
                                &field_path(&section.kind, &section.name, "check_interval"),
                                section.options.get("check_interval").map(String::as_str),
                                60,
                                1,
                                u64::MAX,
                            )?,
                            force_interval: parse_u64_range(
                                &field_path(&section.kind, &section.name, "force_interval"),
                                section.options.get("force_interval").map(String::as_str),
                                3600,
                                1,
                                u64::MAX,
                            )?,
                            retry_count: parse_u32_range(
                                &field_path(&section.kind, &section.name, "retry_count"),
                                section.options.get("retry_count").map(String::as_str),
                                3,
                                0,
                                u32::MAX,
                            )?,
                            retry_backoff: parse_u64_range(
                                &field_path(&section.kind, &section.name, "retry_backoff"),
                                section.options.get("retry_backoff").map(String::as_str),
                                30,
                                1,
                                u64::MAX,
                            )?,
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

fn get_string(map: &BTreeMap<String, String>, key: &str) -> Option<ConfigText> {
    map.get(key).cloned()
}

fn get_required_string(
    map: &BTreeMap<String, String>,
    key: &str,
    section: &str,
) -> Result<ConfigText> {
    map.get(key).cloned().ok_or_else(|| {
        Error::new(format!(
            "section '{section}' missing required option '{key}'"
        ))
    })
}

const MAIN_OPTIONS: &[&str] = &[
    "enabled",
    "log_dir",
    "state_dir",
    "listen",
    "poll_interval",
    "timeout",
    "log_level",
];
const SOURCE_OPTIONS: &[&str] = &[
    "name",
    "type",
    "family",
    "interface",
    "address",
    "probe_url",
    "script",
    "duid",
    "iaid",
    "mac",
    "lease_file",
    "prefix_filter",
    "hostname_hint",
];
const PROVIDER_OPTIONS: &[&str] = &[
    "name",
    "type",
    "api_token",
    "secret_id",
    "secret_key",
    "access_key_id",
    "access_key_secret",
    "url",
    "method",
    "headers_json",
    "body_template",
    "lookup_url",
    "lookup_method",
    "lookup_headers_json",
    "lookup_json_pointer",
    "success_contains",
];
const RULE_OPTIONS: &[&str] = &[
    "name",
    "enabled",
    "provider",
    "source",
    "record_type",
    "zone",
    "record_name",
    "ttl",
    "proxied",
    "check_interval",
    "force_interval",
    "retry_count",
    "retry_backoff",
];

fn reject_unknown_options(
    kind: &str,
    name: &str,
    options: &BTreeMap<String, String>,
    allowed: &[&str],
) -> Result<()> {
    for option in options.keys() {
        if !allowed.contains(&option.as_str()) {
            return Err(Error::new(format!(
                "unknown option '{}'",
                field_path(kind, name, option)
            )));
        }
    }
    Ok(())
}

fn field_path(kind: &str, name: &str, key: &str) -> String {
    if kind == "qddns" {
        format!("main.{key}")
    } else {
        format!("{kind}.{name}.{key}")
    }
}

fn parse_source_kind(
    section: &str,
    source_type: &str,
    options: &BTreeMap<String, String>,
) -> Result<SourceKind> {
    let family = parse_optional_family(options.get("family").map(String::as_str))?;
    match source_type {
        "local_addr" => Ok(SourceKind::LocalAddr {
            family,
            address: get_string(options, "address"),
        }),
        "interface" => Ok(SourceKind::Interface {
            family,
            interface: get_string(options, "interface"),
        }),
        "public_probe" => {
            let probe_url = required_source_option(options, section, "probe_url")?;
            validate_optional_url(
                &field_path("source", section, "probe_url"),
                Some(probe_url.as_str()),
            )?;
            Ok(SourceKind::PublicProbe {
                family,
                probe_url: Some(probe_url),
            })
        }
        "script" => Ok(SourceKind::Script {
            family,
            script: get_string(options, "script"),
        }),
        "dhcpv6_duid" => Ok(SourceKind::Dhcpv6Duid {
            duid: get_string(options, "duid"),
            iaid: get_string(options, "iaid"),
            interface: get_string(options, "interface"),
            lease_file: get_string(options, "lease_file"),
            prefix_filter: get_string(options, "prefix_filter"),
            hostname_hint: get_string(options, "hostname_hint"),
        }),
        "dhcpv6_mac" => Ok(SourceKind::Dhcpv6Mac {
            mac: get_string(options, "mac"),
            interface: get_string(options, "interface"),
            lease_file: get_string(options, "lease_file"),
            prefix_filter: get_string(options, "prefix_filter"),
            hostname_hint: get_string(options, "hostname_hint"),
        }),
        other => Err(Error::new(format!(
            "source '{section}' has unsupported type '{other}'"
        ))),
    }
}

fn parse_provider_kind(
    section: &str,
    provider_type: &str,
    options: &BTreeMap<String, String>,
) -> Result<ProviderKind> {
    match provider_type {
        "cloudflare" => Ok(ProviderKind::Cloudflare {
            api_token: Some(required_provider_option(options, section, "api_token")?),
        }),
        "dnspod" => Ok(ProviderKind::DnsPod {
            secret_id: Some(required_provider_option(options, section, "secret_id")?),
            secret_key: Some(required_provider_option(options, section, "secret_key")?),
        }),
        "aliyun" => Ok(ProviderKind::Aliyun {
            access_key_id: Some(required_provider_option(options, section, "access_key_id")?),
            access_key_secret: Some(required_provider_option(
                options,
                section,
                "access_key_secret",
            )?),
        }),
        "custom_http" => {
            let url = get_string(options, "url");
            let lookup_url = get_string(options, "lookup_url");
            validate_optional_url(&field_path("provider", section, "url"), url.as_deref())?;
            validate_optional_url(
                &field_path("provider", section, "lookup_url"),
                lookup_url.as_deref(),
            )?;
            Ok(ProviderKind::CustomHttp(CustomHttpConfig {
                url,
                method: get_string(options, "method"),
                headers_json: get_string(options, "headers_json"),
                body_template: get_string(options, "body_template"),
                lookup_url,
                lookup_method: get_string(options, "lookup_method"),
                lookup_headers_json: get_string(options, "lookup_headers_json"),
                lookup_json_pointer: get_string(options, "lookup_json_pointer"),
                success_contains: get_string(options, "success_contains"),
            }))
        }
        other => Err(Error::new(format!("unsupported provider type '{other}'"))),
    }
}

fn parse_optional_family(value: Option<&str>) -> Result<Option<AddressFamily>> {
    value.map(AddressFamily::parse).transpose()
}

fn required_provider_option(
    options: &BTreeMap<String, String>,
    section: &str,
    key: &str,
) -> Result<ConfigText> {
    get_string(options, key).filter(|value| !value.trim().is_empty()).ok_or_else(|| {
        Error::new(format!(
            "provider.{section}.{key} missing required credential; rebuild this provider in LuCI or set the new field explicitly"
        ))
    })
}

fn required_source_option(
    options: &BTreeMap<String, String>,
    section: &str,
    key: &str,
) -> Result<ConfigText> {
    get_string(options, key)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| Error::new(format!("source.{section}.{key} missing required option")))
}

fn validate_optional_url(path: &str, value: Option<&str>) -> Result<()> {
    let Some(value) = value else {
        return Ok(());
    };
    if value.starts_with("http://") || value.starts_with("https://") {
        Ok(())
    } else {
        Err(Error::new(format!(
            "{path} has unsupported URL scheme; only http:// and https:// are accepted"
        )))
    }
}

fn parse_bool(path: &str, value: Option<&str>, default: bool) -> Result<bool> {
    match value {
        Some("1") | Some("true") | Some("yes") | Some("on") => Ok(true),
        Some("0") | Some("false") | Some("no") | Some("off") => Ok(false),
        Some(raw) => Err(Error::new(format!("{path} has invalid bool value '{raw}'"))),
        None => Ok(default),
    }
}

fn parse_u64_range(
    path: &str,
    value: Option<&str>,
    default: u64,
    min: u64,
    max: u64,
) -> Result<u64> {
    let parsed = match value {
        Some(raw) => raw
            .parse::<u64>()
            .map_err(|_| Error::new(format!("{path} has invalid integer value '{raw}'")))?,
        None => default,
    };
    if parsed < min || parsed > max {
        Err(Error::new(format!(
            "{path} must be between {min} and {max}, got {parsed}"
        )))
    } else {
        Ok(parsed)
    }
}

fn parse_u32_range(
    path: &str,
    value: Option<&str>,
    default: u32,
    min: u32,
    max: u32,
) -> Result<u32> {
    let parsed = match value {
        Some(raw) => raw
            .parse::<u32>()
            .map_err(|_| Error::new(format!("{path} has invalid integer value '{raw}'")))?,
        None => default,
    };
    if parsed < min || parsed > max {
        Err(Error::new(format!(
            "{path} must be between {min} and {max}, got {parsed}"
        )))
    } else {
        Ok(parsed)
    }
}
