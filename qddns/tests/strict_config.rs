use std::path::Path;

use qddns::config::Config;

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("qddns crate has repo parent")
}

#[test]
fn rejects_invalid_bool_number_and_missing_credentials() {
    let invalid_bool = r#"
config qddns 'main'
    option enabled 'maybe'
"#;
    let err = Config::parse_uci(invalid_bool).expect_err("invalid bool must fail");
    assert!(
        err.to_string().contains("main.enabled"),
        "unexpected error: {err}"
    );

    let invalid_number = r#"
config qddns 'main'
    option timeout 'banana'
"#;
    let err = Config::parse_uci(invalid_number).expect_err("invalid number must fail");
    assert!(
        err.to_string().contains("main.timeout"),
        "unexpected error: {err}"
    );

    let missing_credentials = r#"
config qddns 'main'

config source 'wan4'
    option type 'local_addr'
    option family 'ipv4'
    option address '198.51.100.10'

config provider 'cf'
    option type 'cloudflare'

config rule 'home'
    option provider 'cf'
    option source 'wan4'
    option record_type 'A'
    option zone 'example.com'
    option record_name 'home'
"#;
    let err = Config::parse_uci(missing_credentials).expect_err("missing credential must fail");
    assert!(
        err.to_string().contains("provider.cf.api_token"),
        "unexpected error: {err}"
    );
}

#[test]
fn strict_config_accepts_valid_fixture() {
    let root = repo_root();
    for path in [
        root.join("net/qddns/files/qddns.config"),
        root.join("tests/selftest.conf"),
    ] {
        let config = Config::load_from_path(&path)
            .unwrap_or_else(|err| panic!("{} should load: {err}", path.display()));
        config
            .validate()
            .unwrap_or_else(|err| panic!("{} should validate: {err}", path.display()));
    }
}

#[test]
fn public_probe_requires_probe_url_and_rejects_legacy_url_alias() {
    let legacy_url = r#"
config qddns 'main'

config source 'probe'
    option type 'public_probe'
    option family 'ipv4'
    option url 'http://127.0.0.1/probe'
"#;
    let err = Config::parse_uci(legacy_url).expect_err("legacy url alias must fail");
    assert!(
        err.to_string().contains("source.probe.url"),
        "unexpected error: {err}"
    );

    let missing_probe_url = r#"
config qddns 'main'

config source 'probe'
    option type 'public_probe'
    option family 'ipv4'
"#;
    let err = Config::parse_uci(missing_probe_url).expect_err("missing probe_url must fail");
    assert!(
        err.to_string().contains("source.probe.probe_url"),
        "unexpected error: {err}"
    );
}

#[test]
fn rule_accepts_optional_probe_interface_and_validates_name() {
    let config_text = r#"
config qddns 'main'

config source 'probe'
    option type 'public_probe'
    option family 'ipv4'
    option probe_url 'http://127.0.0.1/probe'

config provider 'cf'
    option type 'cloudflare'
    option api_token 'token'

config rule 'home'
    option provider 'cf'
    option source 'probe'
    option record_type 'A'
    option zone 'example.com'
    option record_name 'home'
    option probe_interface 'wan2'
"#;
    let config = Config::parse_uci(config_text).expect("probe_interface should parse");
    assert_eq!(
        config.rules["home"].probe_interface.as_deref(),
        Some("wan2")
    );

    let invalid = config_text.replace("wan2", "wan2;reboot");
    let config = Config::parse_uci(&invalid).expect("invalid probe_interface should parse");
    let err = config.validate().expect_err("invalid interface name must fail");
    assert!(
        err.to_string().contains("rule.home.probe_interface"),
        "unexpected error: {err}"
    );
}

#[test]
fn luci_schema_matches_typed_config() {
    let root = repo_root();
    let settings =
        std::fs::read_to_string(root.join(
            "applications/luci-app-qddns/htdocs/luci-static/resources/view/qddns/settings.js",
        ))
        .expect("read settings.js");
    let rules = std::fs::read_to_string(
        root.join("applications/luci-app-qddns/htdocs/luci-static/resources/view/qddns/rules.js"),
    )
    .expect("read rules.js");

    assert!(
        settings.contains("form.ListValue, 'family'"),
        "source family must be a closed ipv4/ipv6 selector"
    );
    assert!(
        settings.contains("range(1, 30)") || settings.contains("range(1,30)"),
        "timeout must match the core 1..30 second range"
    );
    assert!(
        rules.contains("'retry_count'"),
        "rules form must expose retry_count because core retry policy uses it"
    );
}
