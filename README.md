# qddns

`qddns` is a new OpenWrt/ImmortalWrt DDNS platform built from scratch around a Rust backend and a LuCI control panel.

## Layout

- `qddns/`
  Rust library plus `qddnsctl` and `qddnsd`
- `net/qddns/`
  OpenWrt package for the backend daemon, CLI, init script, and default UCI config
- `applications/luci-app-qddns/`
  LuCI view, menu entry, ACL, and rpcd ucode bridge

## Current capabilities

- UCI config parsing and validation
- Source resolution for:
  - `local_addr`
  - `interface`
  - `public_probe`
  - `script`
  - `dhcpv6_duid`
- Runtime state persistence in `runtime.state`
- Rule execution state machine with per-rule logs
- Provider adapters for:
  - `cloudflare`
  - `dnspod`
  - `aliyun`
  - `custom_http`
- CLI:
  - `qddnsctl status`
  - `qddnsctl validate`
  - `qddnsctl sources list`
  - `qddnsctl sources probe <id>`
  - `qddnsctl rules list`
  - `qddnsctl rules run <id>`
  - `qddnsctl rules test <id>`
  - `qddnsctl rules status <id>`
- Daemon scheduler with `--once` batch run and polling loop
- LuCI overview console with source probing, rule actions, runtime status, and editable UCI sections

## Runtime requirements

- OpenWrt `procd`
- `ucode`, `ucode-mod-fs`, and `ucode-mod-uci`
- Rust standard runtime for the target architecture

Core HTTP, JSON, HMAC/signing, and UTC timestamp handling are implemented inside the Rust backend. The backend no longer shells out to external network, crypto, or date utilities during normal operation.

## Rust dependencies

The backend intentionally uses small blocking dependencies instead of a large async stack:

- `serde` and `serde_json` for runtime/provider/CLI JSON contracts
- `ureq` with rustls-backed TLS for blocking HTTP/HTTPS
- `hmac`, `sha1`, `sha2`, `hex`, and `base64` for provider signing
- `percent-encoding` for canonical query construction
- `time` for UTC timestamp formatting

The OpenWrt package does not need runtime dependencies for external HTTP clients, OpenSSL command-line tools, or coreutils date utilities.

## Breaking config notes

Configuration parsing is strict. Unknown options, invalid booleans/numbers, unsupported URL schemes, and missing provider credentials now fail validation with field-path errors such as `provider.cf.api_token: missing`.

Production `custom_http` provider URLs and `public_probe` source URLs must use `http://` or `https://`; `file://` is rejected. The legacy `command` source type is no longer accepted, and LuCI/rpcd source probing is limited to local, interface, and DHCPv6 DUID sources.

## Verification

```sh
cd qddns && CARGO_TARGET_DIR=/tmp/qddns-cargo-target cargo test -p qddns -- --nocapture
cd .. && bash tests/verify.sh
for f in applications/luci-app-qddns/htdocs/luci-static/resources/view/qddns/*.js; do node --check "$f"; done
python3 tests/check_acl_boundaries.py
python3 tests/check_rpcd_redaction.py
```
