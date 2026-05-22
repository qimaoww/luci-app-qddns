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
  - `command`
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

- `curl`
- `openssl`
- `date` with `-r <epoch>` support

The OpenWrt package definition pulls these in as `curl`, `openssl-util`, and `coreutils-date`.

## Verification

```sh
cargo test -p qddns
```

The Rust backend intentionally stays standard-library-only so it can be built in restricted environments without fetching extra crates.
