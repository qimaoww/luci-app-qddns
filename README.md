# qddns

`qddns` 是一个面向 OpenWrt/ImmortalWrt 的 DDNS 平台，由 Rust 后端和 LuCI 控制面板组成。

## 目录结构

- `qddns/`
  Rust 库，以及 `qddnsctl`、`qddnsd`
- `net/qddns/`
  后端守护进程、命令行工具、启动脚本和默认 UCI 配置的 OpenWrt 软件包
- `applications/luci-app-qddns/`
  LuCI 页面、菜单入口、ACL 和 rpcd ucode 桥接

## 当前功能

- UCI 配置解析和校验
- 来源 IP 解析：
  - `local_addr`
  - `interface`
  - `public_probe`
  - `script`
  - `dhcpv6_duid`
  - `dhcpv6_mac`
- 运行态持久化到 `runtime.state`
- 规则执行状态机和按规则记录的日志
- 服务商适配：
  - `cloudflare`
  - `dnspod`
  - `aliyun`
  - `custom_http`
- 命令行工具：
  - `qddnsctl status`
  - `qddnsctl validate`
  - `qddnsctl sources list`
  - `qddnsctl sources probe <id>`
  - `qddnsctl rules list`
  - `qddnsctl rules run <id>`
  - `qddnsctl rules test <id>`
  - `qddnsctl rules status <id>`
- 守护进程调度器，支持 `--once` 批量执行和循环轮询
- LuCI 概览控制台，支持来源 IP 探测、规则操作、运行态查看和 UCI 配置编辑

## 局域网 IPv6 来源

`dhcpv6_duid` 保留严格的 DHCPv6 租约查找路径：先在 `/tmp/odhcpd.leases` 中匹配 DUID 和 IAID，再只接受匹配已配置 WAN/上游接口 DHCPv6-PD 路由来源前缀的公网 IPv6 候选地址。

`dhcpv6_mac` 是独立的基于 MAC 的来源类型。它会规范化 MAC 地址，从 `/tmp/odhcpd.leases` 和 IPv6 邻居表收集候选地址，并在选择前对 IPv6 地址去重。只有位于 `2000::/3`、且匹配已配置 WAN/上游接口 DHCPv6-PD 路由来源前缀的公网 IPv6 会被接受；链路本地地址、ULA 和文档前缀都会被忽略。如果同一主机存在多个匹配的公网 IPv6，QDDNS 会确定性地选择第一个匹配候选；只有在 WAN/PD 来源前缀匹配之后还需要进一步收窄时，才设置 `prefix_filter`，例如 `240e:` 或 `2409:`。`prefix_filter` 不能替代 `interface`。

LuCI 的 MAC 选择器会显示 MAC、主机名、LAN IPv4/私有 IPv4 提示、主机接口和公网 IPv6 前缀。LAN IPv4 和主机接口只用于帮助识别主机，不影响 DDNS IPv6 的有效性；来源配置里的 `interface` 字段仍然表示用于前缀校验的 WAN/上游接口。MAC 选择不会显示、请求或返回 DUID/IAID 字段。选择器直接读取 `/tmp/dhcp.leases`、`/tmp/odhcpd.leases` 以及 IPv4/IPv6 邻居表，不会在 rpcd 内部调用 `luci-rpc`。

## 运行依赖

- OpenWrt `procd`
- `ip-full`
- `ucode`、`ucode-mod-fs` 和 `ucode-mod-uci`
- 目标架构对应的 Rust 标准运行时

核心 HTTP、JSON、HMAC/签名和 UTC 时间戳处理都在 Rust 后端内部实现。后端在正常运行时不再调用外部网络、加密或日期工具。

## Rust 依赖

后端有意使用小型阻塞依赖，而不是引入大型异步技术栈：

- `serde` 和 `serde_json` 用于运行态、服务商和 CLI 的 JSON 契约
- `ureq` 结合 rustls TLS 支持，用于阻塞式 HTTP/HTTPS
- `hmac`、`sha1`、`sha2`、`hex` 和 `base64` 用于服务商签名
- `percent-encoding` 用于构造规范化查询参数
- `time` 用于格式化 UTC 时间戳

OpenWrt 软件包不需要外部 HTTP 客户端、OpenSSL 命令行工具或 coreutils 日期工具作为运行时依赖。

## 配置兼容性说明

配置解析是严格模式。未知选项、非法布尔值/数字、不支持的 URL scheme、缺失的服务商凭据都会导致校验失败，并返回带字段路径的错误，例如 `provider.cf.api_token: missing`。

生产环境的 `custom_http` 服务商 URL 和 `public_probe` 来源 URL 必须使用 `http://` 或 `https://`；`file://` 会被拒绝。旧版 `command` 来源类型不再接受，LuCI/rpcd 来源探测仅限本地地址、接口、DHCPv6 DUID 和 MAC 来源。

## 验证

```sh
cd qddns && CARGO_TARGET_DIR=/tmp/qddns-cargo-target cargo test -p qddns -- --nocapture
cd .. && bash tests/verify.sh
for f in applications/luci-app-qddns/htdocs/luci-static/resources/view/qddns/*.js; do node --check "$f"; done
python3 tests/check_acl_boundaries.py
python3 tests/check_rpcd_redaction.py
```
