#!/usr/bin/env python3
import pathlib
import re
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]
RPCD = ROOT / "applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
TEXT = RPCD.read_text()


def fail(message: str) -> None:
    print(message, file=sys.stderr)
    raise SystemExit(1)


secret_fields = [
    "api_token",
    "api_key",
    "secret_id",
    "secret_key",
    "access_key_id",
    "access_key_secret",
    "password",
    "headers_json",
    "lookup_headers_json",
    "body_template",
]

for field in secret_fields:
    if re.search(rf"{field}\s*:\s*section\.{field}", TEXT):
        fail(f"rpcd exposes secret field: {field}")

if "function shell_quote" in TEXT:
    fail("rpcd must not rely on shell_quote for dynamic qddnsctl arguments")

if "is_valid_id" not in TEXT or "is_probe_allowed_source_type" not in TEXT:
    fail("rpcd must validate ids and source probe types")

if (
    "source_type == 'command'" in TEXT
    or "source_type == 'script'" in TEXT
    or "source_type == 'public_probe'" in TEXT
):
    fail("rpcd probe allowlist must not include command/script/public_probe")

for command in re.findall(r"exec_json\(`([^`]+)`\)", TEXT):
    if "shell_quote" in command or "${" in command and not any(
        command.startswith(prefix) for prefix in ["sources probe ", "rules run ", "rules test ", "rules status ", "logs "]
    ):
        fail(f"unexpected dynamic exec_json command: {command}")

print("rpcd redaction ok")
