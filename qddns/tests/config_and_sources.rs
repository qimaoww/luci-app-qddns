use std::fs;
use std::net::IpAddr;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use qddns::config::{AddressFamily, Config, SourceConfig, SourceKind};
use qddns::source::resolve_source;

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

fn write_file(path: &Path, content: &str) {
    fs::write(path, content).expect("write fixture");
}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let seq = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("qddns-test-{}-{unique}-{seq}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn loads_uci_config_and_indexes_sections_by_name() {
    let temp = TempDir::new();
    let path = temp.path().join("qddns.conf");
    write_file(
        &path,
        r#"
config qddns 'main'
    option enabled '1'
    option log_dir '/tmp/qddns-log'
    option state_dir '/tmp/qddns-state'
    option listen '127.0.0.1:53530'
    option poll_interval '30'
    option timeout '12'

config source 'lan_duid'
    option type 'dhcpv6_duid'
    option duid '0001000130555374bcfce78c41cb'
    option iaid '6bcfce7'
    option interface 'wan6'
    option prefix_filter '240e:'

config source 'lan_mac'
    option type 'dhcpv6_mac'
    option mac 'bc-fc-e7-8c-41-cb'
    option interface 'wan6'
    option prefix_filter '240e:'

config provider 'cf'
    option type 'cloudflare'
    option api_token 'token'

config rule 'desktop_ipv6'
    option enabled '1'
    option provider 'cf'
    option source 'lan_duid'
    option record_type 'AAAA'
    option zone 'example.com'
    option record_name 'desktop'
    option ttl '300'
    option proxied '0'
    option check_interval '60'
    option force_interval '3600'
    option retry_count '3'
    option retry_backoff '30'
"#,
    );

    let config = Config::load_from_path(&path).expect("config loads");
    assert_eq!(config.main.log_dir, "/tmp/qddns-log");
    assert_eq!(config.sources.len(), 2);
    assert_eq!(config.providers.len(), 1);
    assert_eq!(config.rules.len(), 1);
    let SourceKind::Dhcpv6Duid {
        interface,
        prefix_filter,
        ..
    } = &config.sources["lan_duid"].kind
    else {
        panic!("lan_duid should be dhcpv6_duid");
    };
    assert_eq!(interface.as_deref(), Some("wan6"));
    // The interface prefix is the primary validity source; prefix_filter only narrows it.
    assert_eq!(prefix_filter.as_deref(), Some("240e:"));
    let SourceKind::Dhcpv6Mac {
        mac,
        interface,
        prefix_filter,
        ..
    } = &config.sources["lan_mac"].kind
    else {
        panic!("lan_mac should be dhcpv6_mac");
    };
    assert_eq!(mac.as_deref(), Some("bc-fc-e7-8c-41-cb"));
    assert_eq!(interface.as_deref(), Some("wan6"));
    // The interface prefix is the primary validity source; prefix_filter only narrows it.
    assert_eq!(prefix_filter.as_deref(), Some("240e:"));
    assert_eq!(config.rules["desktop_ipv6"].record_type, "AAAA");
}

#[test]
fn dhcpv6_source_requires_interface() {
    let config = Config::parse_uci(
        r#"
config qddns 'main'

config source 'lan_duid'
    option type 'dhcpv6_duid'
    option duid '0001000130555374bcfce78c41cb'
    option iaid '6bcfce7'

config source 'lan_mac'
    option type 'dhcpv6_mac'
    option mac 'bc-fc-e7-8c-41-cb'

config provider 'cf'
    option type 'cloudflare'
    option api_token 'token'

config rule 'desktop_ipv6'
    option enabled '1'
    option provider 'cf'
    option source 'lan_duid'
    option record_type 'AAAA'
    option zone 'example.com'
    option record_name 'desktop'
"#,
    )
    .expect("config parses");

    let err = config
        .validate()
        .expect_err("missing dhcpv6 interface must fail");
    assert!(
        err.to_string().contains("source.lan_duid.interface"),
        "unexpected error: {err}"
    );
}

#[test]
fn validation_rejects_aaaa_rule_bound_to_ipv4_local_addr_source() {
    let temp = TempDir::new();
    let path = temp.path().join("qddns.conf");
    write_file(
        &path,
        r#"
config qddns 'main'

config source 'ipv4_local'
    option type 'local_addr'
    option address '192.168.1.10'

config provider 'cf'
    option type 'cloudflare'
    option api_token 'token'

config rule 'bad_ipv6'
    option enabled '1'
    option provider 'cf'
    option source 'ipv4_local'
    option record_type 'AAAA'
    option zone 'example.com'
    option record_name 'bad'
    option ttl '300'
    option proxied '0'
    option check_interval '60'
    option force_interval '3600'
    option retry_count '3'
    option retry_backoff '30'
"#,
    );

    let config = Config::load_from_path(&path).expect("config loads");
    let err = config.validate().expect_err("validation should fail");
    assert!(err.to_string().contains("AAAA"), "unexpected error: {err}");
}

#[test]
fn script_source_runs_local_script_and_returns_ipv6() {
    let temp = TempDir::new();
    let script_path = temp.path().join("source.sh");
    write_file(&script_path, "#!/bin/sh\nprintf 2001:db8::23\n");
    let mut perms = fs::metadata(&script_path).unwrap().permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).unwrap();
    }

    let source = SourceConfig {
        name: "script_ip".into(),
        kind: SourceKind::Script {
            family: Some(AddressFamily::Ipv6),
            script: Some(script_path.display().to_string()),
        },
    };

    let resolved = resolve_source(&source).expect("script source resolves");
    assert_eq!(resolved.address, "2001:db8::23".parse::<IpAddr>().unwrap());
}

#[test]
fn interface_source_resolves_loopback_ipv4() {
    let source = SourceConfig {
        name: "loopback".into(),
        kind: SourceKind::Interface {
            family: Some(AddressFamily::Ipv4),
            interface: Some("lo".into()),
        },
    };

    let resolved = resolve_source(&source).expect("interface source resolves");
    assert_eq!(resolved.address, "127.0.0.1".parse::<IpAddr>().unwrap());
}
