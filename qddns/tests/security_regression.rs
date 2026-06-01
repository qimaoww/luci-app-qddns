mod support;

use std::path::Path;

#[test]
fn security_test_helpers_smoke() {
    let temp = support::TempDir::new("qddns-security-smoke");
    assert!(temp.path().is_dir());
    let fixture = temp.write("fixture.txt", "ok");
    assert_eq!(std::fs::read_to_string(fixture).unwrap(), "ok");

    match support::MockHttpServer::try_single_response(200, "ok") {
        Ok(server) => {
            let body = ureq::get(&server.url("/probe"))
                .call()
                .expect("mock response")
                .body_mut()
                .read_to_string()
                .expect("response body");
            assert_eq!(body, "ok");

            let requests = server.requests();
            assert_eq!(requests.len(), 1);
            assert_eq!(requests[0].method, "GET");
            assert_eq!(requests[0].path, "/probe");
            assert_eq!(requests[0].body, "");
        }
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("local TCP bind unavailable in this sandbox; mock helper remains available");
        }
        Err(err) => panic!("bind mock server: {err}"),
    }

    support::assert_secret_absent("test-secret", &[b"plain output"]);

    let _run_helper: fn(&std::path::Path, &[&str]) -> std::process::Output = support::run_qddnsctl;
}

#[test]
fn rejects_file_scheme_provider() {
    let pwned = std::path::Path::new("/tmp/qddns-file-provider-pwned");
    let _ = std::fs::remove_file(pwned);

    let temp = support::TempDir::new("qddns-file-scheme");
    let state_dir = temp.path().join("state");
    let log_dir = temp.path().join("logs");
    std::fs::create_dir_all(&state_dir).unwrap();
    std::fs::create_dir_all(&log_dir).unwrap();
    let config_path = temp.write(
        "qddns.conf",
        &format!(
            r#"
config qddns 'main'
    option enabled '1'
    option state_dir '{}'
    option log_dir '{}'

config source 'wan4'
    option type 'local_addr'
    option address '198.51.100.77'

config provider 'custom'
    option type 'custom_http'
    option url 'file://{}'
    option method 'POST'
    option body_template 'owned {{{{ip}}}}'
    option success_contains 'owned'

config rule 'home'
    option enabled '1'
    option provider 'custom'
    option source 'wan4'
    option record_type 'A'
    option zone 'example.com'
    option record_name 'home'
    option ttl '300'
    option check_interval '60'
    option force_interval '3600'
    option retry_count '3'
    option retry_backoff '30'
"#,
            state_dir.display(),
            log_dir.display(),
            pwned.display()
        ),
    );

    let err = qddns::daemon::run_rule_once(config_path.to_str().unwrap(), "home")
        .expect_err("file provider scheme must be rejected");
    assert!(
        err.to_string().contains("provider.custom.url")
            && err.to_string().contains("unsupported URL scheme"),
        "unexpected error: {err}"
    );
    assert!(!pwned.exists(), "file provider created {}", pwned.display());
}

#[test]
fn rejects_interface_shell_metacharacters() {
    let pwned = Path::new("/tmp/qddns-pwned");
    let attacks = [
        "wan; touch /tmp/qddns-pwned",
        "$(touch /tmp/qddns-pwned)",
        "wan | touch /tmp/qddns-pwned",
        "wan > /tmp/qddns-pwned",
        "bad iface",
    ];

    for iface in attacks {
        let _ = std::fs::remove_file(pwned);
        let source = qddns::config::SourceConfig {
            name: "bad_interface".into(),
            kind: qddns::config::SourceKind::Interface {
                family: Some(qddns::config::AddressFamily::Ipv4),
                interface: Some(iface.into()),
            },
        };

        let err = qddns::source::resolve_source(&source)
            .expect_err("malicious interface name must be rejected");
        assert!(
            err.to_string().contains("invalid interface name"),
            "unexpected error for {iface:?}: {err}"
        );
        assert!(
            !pwned.exists(),
            "malicious interface name {iface:?} created {}",
            pwned.display()
        );
    }
}

#[test]
fn rejects_command_source_type_in_config() {
    let err = qddns::config::Config::parse_uci(
        r#"
config qddns 'main'

config source 'cmd'
    option type 'command'
    option family 'ipv4'
    option command 'touch /tmp/qddns-pwned; printf 198.51.100.7'
"#,
    )
    .expect_err("command source type must be rejected");

    assert!(
        err.to_string().contains("source.cmd.command")
            || err.to_string().contains("unsupported type 'command'"),
        "unexpected error: {err}"
    );
}

#[test]
fn public_probe_rejects_file_scheme_and_does_not_read_local_file() {
    let temp = support::TempDir::new("qddns-public-probe-file");
    let secret_path = temp.write("secret.txt", "203.0.113.88\n");
    let source = qddns::config::SourceConfig {
        name: "probe_ip".into(),
        kind: qddns::config::SourceKind::PublicProbe {
            family: Some(qddns::config::AddressFamily::Ipv4),
            probe_url: Some(format!("file://{}", secret_path.display())),
        },
    };
    let http = qddns::http::HttpClient::from_timeout_secs(1);

    let err = qddns::source::resolve_source_with_http(&source, &http)
        .expect_err("public probe must reject file scheme");
    assert!(
        err.to_string().contains("unsupported probe URL scheme"),
        "unexpected error: {err}"
    );
}

#[test]
fn rpcd_probe_rejects_script() {
    let pwned = Path::new("/tmp/qddns-rpc-pwned");
    let _ = std::fs::remove_file(pwned);

    let temp = support::TempDir::new("qddns-rpc-probe");
    let state_dir = temp.path().join("state");
    let log_dir = temp.path().join("logs");
    std::fs::create_dir_all(&state_dir).unwrap();
    std::fs::create_dir_all(&log_dir).unwrap();
    let script_path = temp.path().join("rpc-pwn.sh");
    std::fs::write(
        &script_path,
        "#!/bin/sh\ntouch /tmp/qddns-rpc-pwned\nprintf 198.51.100.7\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script_path, perms).unwrap();
    }

    let config_path = temp.write(
        "qddns.conf",
        &format!(
            r#"
config qddns 'main'
    option enabled '1'
    option state_dir '{}'
    option log_dir '{}'

config source 'scripted'
    option type 'script'
    option family 'ipv4'
    option script '{}'
"#,
            state_dir.display(),
            log_dir.display(),
            script_path.display()
        ),
    );

    let err = qddns::daemon::probe_source(config_path.to_str().unwrap(), "scripted")
        .expect_err("rpcd source probe must reject script source type");
    assert!(
        err.to_string()
            .contains("probe not allowed for source type"),
        "unexpected error: {err}"
    );

    assert!(!pwned.exists(), "rpcd probe executed dangerous source");
}
