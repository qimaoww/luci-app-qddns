use std::fs;
use std::net::{IpAddr, Ipv6Addr};
use std::path::Path;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use qddns::config::Config;
use qddns::source::resolve_source;

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
        let path = std::env::temp_dir().join(format!("qddns-test-{unique}"));
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
    assert_eq!(config.sources.len(), 1);
    assert_eq!(config.providers.len(), 1);
    assert_eq!(config.rules.len(), 1);
    assert_eq!(
        config.sources["lan_duid"].prefix_filter.as_deref(),
        Some("240e:")
    );
    assert_eq!(config.rules["desktop_ipv6"].record_type, "AAAA");
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
    assert!(
        err.to_string().contains("AAAA"),
        "unexpected error: {err}"
    );
}

#[test]
fn duid_source_prefers_matching_prefix_when_multiple_global_addresses_exist() {
    let temp = TempDir::new();
    let lease_path = temp.path().join("odhcpd.leases");
    write_file(
        &lease_path,
        "# br-lan 0001000130555374bcfce78c41cb 6bcfce7 DESKTOP-DVAVJOS 1778918207 30 128 240e:3b2:4e8f:bf40::30/128 2409:8a55:4e29:d250::30/128\n",
    );

    let config = Config::load_from_path(Path::new("/dev/null")).unwrap_or_default();
    let mut source = config.sources.get("missing").cloned().unwrap_or_else(|| qddns::config::SourceConfig {
        name: "lan_duid".into(),
        source_type: "dhcpv6_duid".into(),
        family: None,
        interface: None,
        address: None,
        probe_url: None,
        script: None,
        command: None,
        duid: Some("0001000130555374bcfce78c41cb".into()),
        iaid: Some("6bcfce7".into()),
        lease_file: Some(lease_path.display().to_string()),
        prefix_filter: Some("240e:".into()),
        hostname_hint: Some("DESKTOP-DVAVJOS".into()),
    });
    source.lease_file = Some(lease_path.display().to_string());

    let resolved = resolve_source(&source).expect("duid source resolves");
    assert_eq!(
        resolved.address,
        IpAddr::V6("240e:3b2:4e8f:bf40::30".parse::<Ipv6Addr>().unwrap())
    );
}

#[test]
fn duid_source_rejects_ambiguous_global_addresses_without_prefix_filter() {
    let temp = TempDir::new();
    let lease_path = temp.path().join("odhcpd.leases");
    write_file(
        &lease_path,
        "# br-lan 0001000130555374bcfce78c41cb 6bcfce7 DESKTOP-DVAVJOS 1778918207 30 128 240e:3b2:4e8f:bf40::30/128 2409:8a55:4e29:d250::30/128\n",
    );

    let source = qddns::config::SourceConfig {
        name: "lan_duid".into(),
        source_type: "dhcpv6_duid".into(),
        family: None,
        interface: None,
        address: None,
        probe_url: None,
        script: None,
        command: None,
        duid: Some("0001000130555374bcfce78c41cb".into()),
        iaid: Some("6bcfce7".into()),
        lease_file: Some(lease_path.display().to_string()),
        prefix_filter: None,
        hostname_hint: None,
    };

    let err = resolve_source(&source).expect_err("resolution should fail");
    assert!(
        err.to_string().contains("prefix"),
        "unexpected error: {err}"
    );
}

#[test]
fn command_source_executes_shell_command_and_returns_ipv4() {
    let source = qddns::config::SourceConfig {
        name: "cmd_ip".into(),
        source_type: "command".into(),
        family: Some("ipv4".into()),
        interface: None,
        address: None,
        probe_url: None,
        script: None,
        command: Some("printf 198.51.100.8".into()),
        duid: None,
        iaid: None,
        lease_file: None,
        prefix_filter: None,
        hostname_hint: None,
    };

    let resolved = resolve_source(&source).expect("command source resolves");
    assert_eq!(resolved.address, "198.51.100.8".parse::<IpAddr>().unwrap());
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

    let source = qddns::config::SourceConfig {
        name: "script_ip".into(),
        source_type: "script".into(),
        family: Some("ipv6".into()),
        interface: None,
        address: None,
        probe_url: None,
        script: Some(script_path.display().to_string()),
        command: None,
        duid: None,
        iaid: None,
        lease_file: None,
        prefix_filter: None,
        hostname_hint: None,
    };

    let resolved = resolve_source(&source).expect("script source resolves");
    assert_eq!(resolved.address, "2001:db8::23".parse::<IpAddr>().unwrap());
}

#[test]
fn public_probe_source_reads_file_url_and_extracts_ip() {
    let temp = TempDir::new();
    let probe_path = temp.path().join("probe.txt");
    write_file(&probe_path, "Current IP Address: 203.0.113.44\n");

    let source = qddns::config::SourceConfig {
        name: "probe_ip".into(),
        source_type: "public_probe".into(),
        family: Some("ipv4".into()),
        interface: None,
        address: None,
        probe_url: Some(format!("file://{}", probe_path.display())),
        script: None,
        command: None,
        duid: None,
        iaid: None,
        lease_file: None,
        prefix_filter: None,
        hostname_hint: None,
    };

    let resolved = resolve_source(&source).expect("public probe resolves");
    assert_eq!(resolved.address, "203.0.113.44".parse::<IpAddr>().unwrap());
}

#[test]
fn interface_source_resolves_loopback_ipv4() {
    let source = qddns::config::SourceConfig {
        name: "loopback".into(),
        source_type: "interface".into(),
        family: Some("ipv4".into()),
        interface: Some("lo".into()),
        address: None,
        probe_url: None,
        script: None,
        command: None,
        duid: None,
        iaid: None,
        lease_file: None,
        prefix_filter: None,
        hostname_hint: None,
    };

    let resolved = resolve_source(&source).expect("interface source resolves");
    assert_eq!(resolved.address, "127.0.0.1".parse::<IpAddr>().unwrap());
}
