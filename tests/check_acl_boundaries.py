#!/usr/bin/env python3
import json
import pathlib
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]
ACL = ROOT / "applications/luci-app-qddns/root/usr/share/rpcd/acl.d/luci-app-qddns.json"


def fail(message: str) -> None:
    print(message, file=sys.stderr)
    raise SystemExit(1)


data = json.loads(ACL.read_text())
grant = data.get("luci-app-qddns", {})
read = grant.get("read", {})
write = grant.get("write", {})

read_ubus_grants = read.get("ubus", {})
write_ubus_grants = write.get("ubus", {})
read_ubus = read_ubus_grants.get("qddns", [])
write_ubus = write_ubus_grants.get("qddns", [])
if sorted(read_ubus) != sorted([
    "get_overview",
    "list_sources",
    "probe_source",
    "list_dhcpv6_leases",
    "list_rules",
    "get_logs",
    "get_rule_status",
]):
    fail(f"unexpected read ubus methods: {read_ubus}")

if sorted(write_ubus) != sorted(["run_rule", "test_rule"]):
    fail(f"unexpected write ubus methods: {write_ubus}")

if "luci-rpc" in read_ubus_grants:
    fail(f"rpcd bridge must not call luci-rpc recursively: {read_ubus_grants.get('luci-rpc')}")

files = read.get("file", {})
allowed_files = {
    "/tmp/dhcp.leases": ["read"],
    "/tmp/odhcpd.leases": ["read"],
    "/sbin/ip": ["exec"],
    "/usr/bin/qddnsctl": ["exec"],
}
if files != allowed_files:
    fail(f"unexpected file ACLs: {files}")

for path in files:
    if "qddns" in path and path != "/usr/bin/qddnsctl":
        fail(f"dynamic qddns path must not be in ACL: {path}")
    if "log" in path or "state" in path:
        fail(f"log/state path must not be in ACL: {path}")

if read.get("uci") != ["qddns"] or write.get("uci") != ["qddns"]:
    fail("ACL must only grant qddns UCI access")

print("acl boundaries ok")
