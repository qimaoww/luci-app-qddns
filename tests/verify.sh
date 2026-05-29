#!/bin/sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
ROOT_DIR=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
VIEW_DIR="$ROOT_DIR/applications/luci-app-qddns/htdocs/luci-static/resources/view/qddns"
MENU_FILE="$ROOT_DIR/applications/luci-app-qddns/root/usr/share/luci/menu.d/luci-app-qddns.json"
PO_FILE="$ROOT_DIR/applications/luci-app-qddns/po/zh_Hans/qddns.po"
DEFAULT_CONFIG_FILE="$ROOT_DIR/net/qddns/files/qddns.config"
export VIEW_DIR

run_step() {
	name="$1"
	shift
	printf '==> %s\n' "$name"
	"$@"
}

check_required_view_files() {
	for file in overview rules settings logs shared; do
		test -f "$VIEW_DIR/$file.js"
	done
}

check_package_metadata() {
	grep -nF 'version = "0.2.0"' "$ROOT_DIR/Cargo.toml"
	grep -nF 'version = "0.2.0"' "$ROOT_DIR/qddns/Cargo.toml"
	grep -nF 'PKG_VERSION:=0.2.0' "$ROOT_DIR/net/qddns/Makefile"
	grep -nF '+ip-full' "$ROOT_DIR/net/qddns/Makefile"
	grep -nF 'PKG_VERSION:=0.2.0' "$ROOT_DIR/applications/luci-app-qddns/Makefile"
	grep -nF 'Build/Prepare/luci-app-qddns' "$ROOT_DIR/applications/luci-app-qddns/Makefile"
	grep -nF 'chmod 0755' "$ROOT_DIR/applications/luci-app-qddns/Makefile"
	grep -nF 'chmod 0644' "$ROOT_DIR/applications/luci-app-qddns/Makefile"
	grep -nF 'dhcpv6_mac' "$ROOT_DIR/README.md"
	grep -nF '对 IPv6 地址去重' "$ROOT_DIR/README.md"
	grep -nF 'DHCPv6-PD 路由来源前缀' "$ROOT_DIR/README.md"
	grep -nF '确定性地选择第一个匹配候选' "$ROOT_DIR/README.md"
	grep -nF 'WAN/PD 来源前缀匹配之后还需要进一步收窄' "$ROOT_DIR/README.md"
	grep -nF 'LAN IPv4' "$ROOT_DIR/README.md"
	grep -nF 'IPv4/IPv6 邻居表' "$ROOT_DIR/README.md"
	grep -nF '不会显示、请求或返回 DUID/IAID' "$ROOT_DIR/README.md"
	! grep -nF 'LuCI host hints' "$ROOT_DIR/README.md"
}

check_view_syntax() {
	for file in "$VIEW_DIR"/*.js; do
		node --check "$file"
	done
}

check_menu_child_pages() {
	grep -nF 'admin/services/qddns/overview' "$MENU_FILE"
	grep -nF 'admin/services/qddns/rules' "$MENU_FILE"
	grep -nF 'admin/services/qddns/settings' "$MENU_FILE"
	grep -nF 'admin/services/qddns/logs' "$MENU_FILE"
}

check_po_format() {
	grep -n '^msgid ' "$PO_FILE"
	grep -n '^msgstr ' "$PO_FILE"
}

check_po_core_msgids() {
	for msgid in 'Overview' 'Rules' 'Settings' 'Logs' 'Run once' 'Probe' 'Close' 'Runtime Summary' 'Source Probe' 'Version' 'Success' 'Updated' 'Unchanged' 'Error' 'Failed' 'Invalid' 'Pending' 'Testing' 'Queued' 'Warning' 'OK' 'Synced'; do
		grep -nF "msgid \"$msgid\"" "$PO_FILE"
	done
}

check_po_critical_zh_msgstrs() {
	python3 - "$PO_FILE" <<'PYEOF'
import re
import sys
import ast
from pathlib import Path

po = Path(sys.argv[1]).read_text()
entries = {}
for match in re.finditer(r'^msgid "((?:[^"\\]|\\.)*)"\nmsgstr "((?:[^"\\]|\\.)*)"', po, re.M):
    msgid = ast.literal_eval('"' + match.group(1) + '"')
    msgstr = ast.literal_eval('"' + match.group(2) + '"')
    entries[msgid] = msgstr

required = {
    'Advanced narrowing after WAN/PD source prefix matching; it cannot replace the interface.':
        'WAN/PD 来源前缀匹配后的高级收窄；不能替代接口。',
    'For DHCPv6 DUID/MAC sources, choose WAN/upstream interface(s). QDDNS uses DHCPv6-PD route source prefixes from them; lease cards only fill the LAN host identity.':
        'DHCPv6 DUID/MAC 来源请选择 WAN/上游接口；QDDNS 使用这些接口的 DHCPv6-PD 路由来源前缀，租约卡片只填充局域网主机身份。',
    'For DHCPv6 DUID/MAC sources, choose WAN/upstream interface(s); DHCPv6-PD route source prefixes from those interfaces validate LAN host IPv6 addresses.':
        'DHCPv6 DUID/MAC 来源请选择 WAN/上游接口；这些接口的 DHCPv6-PD 路由来源前缀用于校验局域网主机 IPv6。',
    'Choose a current MAC to fill MAC, LAN IP identity, and hostname hint. Keep the WAN interface selected separately.':
        '选择当前 MAC 来填充 MAC、局域网 IP 识别信息和主机名提示；WAN 接口请单独保持选中。',
    'Selected LAN host MAC has been filled. Keep the WAN interface selected separately.':
        '已填充所选局域网主机 MAC；WAN 接口请单独保持选中。',
    'Host interface':
        '主机接口',
    'WAN/upstream interface':
        'WAN/上游接口',
    'Overview Console':
        '概览控制台',
    'Rule Console':
        '规则控制台',
    'Copy template values into a new provider without exposing credentials in the main table.':
        '将模板值复制到新的提供商中，不在主表格暴露凭据。',
    'Source IP detected: %s. The saved source will be used for this rule.':
        '已探测到来源 IP：%s。将使用该已保存来源创建规则。',
    'Start a short wizard that creates a complete rule with safe defaults. Use the advanced table below for later edits.':
        '启动一个简短向导，用安全默认值创建完整规则。下方高级规则表格用于后续编辑。',
    'Success':
        '成功',
    'Updated':
        '已更新',
    'Unchanged':
        '未变化',
    'Error':
        '错误',
    'Failed':
        '失败',
    'Invalid':
        '无效',
    'Pending':
        '等待中',
    'Testing':
        '测试中',
    'Queued':
        '排队中',
    'Warning':
        '警告',
    'OK':
        '正常',
    'Synced':
        '已同步',
}

for msgid, expected in required.items():
    actual = entries.get(msgid)
    if actual != expected:
        raise SystemExit(f'critical zh translation mismatch for {msgid!r}: {actual!r}')
PYEOF
}

check_view_i18n_hooks() {
	grep -nF "_('Overview Console')" "$VIEW_DIR/overview.js"
	grep -nF "_('Runtime Summary')" "$VIEW_DIR/overview.js"
	grep -nF "_('Version')" "$VIEW_DIR/overview.js"
	grep -nF "_('Rule Console')" "$VIEW_DIR/rules.js"
	grep -nF "_('Run once')" "$VIEW_DIR/rules.js"
	grep -nF "_('Settings')" "$VIEW_DIR/settings.js"
	grep -nF "_('Source Probe')" "$VIEW_DIR/settings.js"
	grep -nF "_('DHCPv6 DUID')" "$VIEW_DIR/settings.js"
	grep -nF "_('Logs')" "$VIEW_DIR/logs.js"
	grep -nF "_('Load Logs')" "$VIEW_DIR/logs.js"
	grep -nF "_('Close')" "$VIEW_DIR/shared.js"
	grep -nF "_('Request failed')" "$VIEW_DIR/shared.js"
	grep -nF "_('N/A')" "$VIEW_DIR/shared.js"
}

check_overview_primary_cards() {
	! grep -nF "_('Recent Activity')" "$VIEW_DIR/overview.js"
	! grep -nF "_('Last Refresh')" "$VIEW_DIR/overview.js"
	! grep -nF "getRecentActivity" "$VIEW_DIR/overview.js"
	! grep -nF 'msgid "Recent Activity"' "$PO_FILE"
	! grep -nF 'msgid "Last Refresh"' "$PO_FILE"
	! grep -nF 'msgid "No activity yet"' "$PO_FILE"
	grep -nF ".qddns-cards{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:var(--qddns-space-3)}" "$VIEW_DIR/overview.js"
	grep -nF ".qddns-cards{grid-template-columns:repeat(2,minmax(0,1fr))}" "$VIEW_DIR/overview.js"
	grep -nF ".qddns-cards{grid-template-columns:1fr}" "$VIEW_DIR/overview.js"
}

check_no_internal_page_nav() {
	! grep -nF 'renderPageNav' "$VIEW_DIR/shared.js"
	for file in "$VIEW_DIR"/*.js; do
		case "$(basename -- "$file")" in
			shared.js) continue ;;
		esac
		! grep -nF 'qddns.renderPageNav(' "$file"
	done
}

check_rules_table_compactness() {
	grep -nF "E('div', { class: 'qddns-wide-form qddns-rules-form' }, [formEl])" "$VIEW_DIR/rules.js"
	grep -nF "overflow-wrap:anywhere" "$VIEW_DIR/rules.js"
	grep -nF ".qddns-wide-form{width:100%;max-width:100%;overflow-x:auto;-webkit-overflow-scrolling:touch}" "$VIEW_DIR/shared.js"
	grep -nF ".qddns-wide-form .cbi-section-table{min-width:var(--qddns-form-table-min);table-layout:auto}" "$VIEW_DIR/shared.js"
	! grep -nF ".qddns-rules-form.qddns-wide-form{width:100%;max-width:100%;overflow-x:visible}" "$VIEW_DIR/rules.js"
	! grep -nF ".qddns-rules-form.qddns-wide-form .cbi-map{width:100%;min-width:0}" "$VIEW_DIR/rules.js"
	! grep -nF ".qddns-rules-form.qddns-wide-form .cbi-section-table{width:100%;min-width:0;table-layout:fixed}" "$VIEW_DIR/rules.js"
	grep -nF -- "--qddns-rule-toggle-width:6.5rem" "$VIEW_DIR/rules.js"
	grep -nF -- "--qddns-rule-type-width:8rem" "$VIEW_DIR/rules.js"
	grep -nF -- "--qddns-rule-action-min:10rem" "$VIEW_DIR/rules.js"
	! grep -nF -- "--qddns-rule-form-min" "$VIEW_DIR/rules.js"
	! grep -nF -- "--qddns-rule-form-max" "$VIEW_DIR/rules.js"
	python3 - <<'PYEOF'
import os
from pathlib import Path
rules = Path(os.environ['VIEW_DIR'], 'rules.js').read_text().splitlines()
fields = ['provider', 'source', 'zone', 'record_name', 'ttl', 'proxied', 'check_interval', 'force_interval', 'retry_backoff']
for field in fields:
    matches = [i for i, line in enumerate(rules) if "s.option(" in line and f"'{field}'" in line]
    if len(matches) != 1:
        raise SystemExit(f'expected exactly one rules field definition for {field}')
    window = rules[matches[0]:matches[0] + 5]
    if not any('o.modalonly = true;' in line for line in window):
        raise SystemExit(f'{field} must be modal-only so the rules main table stays readable')
PYEOF
}

check_overview_boundary() {
	! grep -nE 'runRule|testRule|getRuleStatus|getLogs|form\.(Map|NamedSection|GridSection)' "$VIEW_DIR/overview.js"
}

check_rules_boundary() {
	grep -nE 'qddns\.(runRule|getRuleStatus)' "$VIEW_DIR/rules.js"
	! grep -nF 'qddns.testRule' "$VIEW_DIR/rules.js"
	grep -nF "form.GridSection, 'rule'" "$VIEW_DIR/rules.js"
	! grep -nE "form\.(NamedSection, 'main'|GridSection, 'source'|GridSection, 'provider')" "$VIEW_DIR/rules.js"
}

check_rules_status_badge_tones() {
	grep -nF "const runtimeTone = state.status || (rule.enabled ? 'enabled' : 'disabled');" "$VIEW_DIR/rules.js"
	grep -nF "qddns.renderStatusBadge(runtime, _('Unknown'), runtimeTone)" "$VIEW_DIR/rules.js"
}

check_status_display_labels() {
	grep -nF "function statusLabel(status)" "$VIEW_DIR/shared.js"
	grep -nF "const label = statusLabel(status || fallback) || '-';" "$VIEW_DIR/shared.js"
	grep -nF "statusLabel: statusLabel" "$VIEW_DIR/shared.js"
	grep -nF "resultLabel: function(result)" "$VIEW_DIR/shared.js"
	grep -nF "qddns.resultLabel(item.last_result) || item.last_error || '-'" "$VIEW_DIR/overview.js"
	grep -nF "qddns.statusLabel(result.status) || _('Unknown')" "$VIEW_DIR/rules.js"
	grep -nF "qddns.resultLabel(result.last_result) || _('N/A')" "$VIEW_DIR/rules.js"
	grep -nF "case 'success':" "$VIEW_DIR/shared.js"
	grep -nF "return _('Success');" "$VIEW_DIR/shared.js"
	grep -nF "case 'updated':" "$VIEW_DIR/shared.js"
	grep -nF "return _('Updated');" "$VIEW_DIR/shared.js"
	grep -nF "case 'unchanged':" "$VIEW_DIR/shared.js"
	grep -nF "return _('Unchanged');" "$VIEW_DIR/shared.js"
}

check_rule_wizard() {
	grep -nF "renderRuleWizard" "$VIEW_DIR/rules.js"
	grep -nF "createRuleFromWizard" "$VIEW_DIR/rules.js"
	grep -nF "id: 'qddns-rule-wizard'" "$VIEW_DIR/rules.js"
	grep -nF "_('Guided DDNS rule setup')" "$VIEW_DIR/rules.js"
	grep -nF "_('Start guided setup')" "$VIEW_DIR/rules.js"
	grep -nF "showRuleWizardModal" "$VIEW_DIR/rules.js"
	grep -nF "ui.showModal(_('Guided DDNS rule setup')" "$VIEW_DIR/rules.js"
	grep -nF "data-wizard-panel" "$VIEW_DIR/rules.js"
	grep -nF "_('1. Source')" "$VIEW_DIR/rules.js"
	grep -nF "_('2. DNS')" "$VIEW_DIR/rules.js"
	grep -nF "_('3. Confirm')" "$VIEW_DIR/rules.js"
	grep -nF "_('Confirm and create the rule')" "$VIEW_DIR/rules.js"
	grep -nF "_('Rule name is generated automatically from the record.')" "$VIEW_DIR/rules.js"
	grep -nF "wizardRuleName" "$VIEW_DIR/rules.js"
	grep -nF "_('Next')" "$VIEW_DIR/rules.js"
	grep -nF "_('Back')" "$VIEW_DIR/rules.js"
	grep -nF "_('Choose the source IP first, then choose the DNS location.')" "$VIEW_DIR/rules.js"
	grep -nF "_('Source setup')" "$VIEW_DIR/rules.js"
	grep -nF "_('Create new source')" "$VIEW_DIR/rules.js"
	grep -nF "_('Use saved source')" "$VIEW_DIR/rules.js"
	grep -nF "_('Probe source IP')" "$VIEW_DIR/rules.js"
	grep -nF "_('Probing source IP...')" "$VIEW_DIR/rules.js"
	grep -nF "_('Source IP detected: %s. The source will be saved with the rule.')" "$VIEW_DIR/rules.js"
	grep -nF "_('Source IP detected: %s. The saved source will be used for this rule.')" "$VIEW_DIR/rules.js"
	grep -nF "_('Unable to read source IP. Choose another source or fix the source configuration.')" "$VIEW_DIR/rules.js"
	grep -nF "_('This source type cannot be previewed in LuCI. Confirm the record type manually; the backend will validate it when the rule runs.')" "$VIEW_DIR/rules.js"
	grep -nF "renderWizardSourceIp" "$VIEW_DIR/rules.js"
	grep -nF "updateWizardSourceProbe" "$VIEW_DIR/rules.js"
	grep -nF "saveNewSource" "$VIEW_DIR/rules.js"
	grep -nF "loadWizardLeases" "$VIEW_DIR/rules.js"
	grep -nF "data-source-ip-status" "$VIEW_DIR/rules.js"
	grep -nF "data-source-ip-error" "$VIEW_DIR/rules.js"
	grep -nF "data-source-create-dirty" "$VIEW_DIR/rules.js"
	grep -nF "sourceProbe.token++" "$VIEW_DIR/rules.js"
	grep -nF "if (token !== sourceProbe.token)" "$VIEW_DIR/rules.js"
	grep -nF "isProbeableSourceType" "$VIEW_DIR/rules.js"
	grep -nF "sourceProbe.loading" "$VIEW_DIR/rules.js"
	grep -nF "qddns.listDhcpv6Leases(mode)" "$VIEW_DIR/rules.js"
	grep -nF "qddns.probeSourceDraft(sourceData)" "$VIEW_DIR/rules.js"
	grep -nF "sourceVersion !== sourceCreate.version" "$VIEW_DIR/rules.js"
	grep -nF "uci.add('qddns', 'source', sectionId)" "$VIEW_DIR/rules.js"
	grep -nF "uci.set('qddns', sectionId, 'type', sourceData.type)" "$VIEW_DIR/rules.js"
	grep -nF "setSourceOption(option, sourceOptionValue(sourceData, option))" "$VIEW_DIR/rules.js"
	grep -nF "fields.source.addEventListener('change'" "$VIEW_DIR/rules.js"
	grep -nF "fields.recordType.addEventListener('change'" "$VIEW_DIR/rules.js"
	grep -nF "_('Source IP')" "$VIEW_DIR/rules.js"
	grep -nF "_('Loading...')" "$VIEW_DIR/rules.js"
	grep -nF "_('Source IP is still loading.')" "$VIEW_DIR/rules.js"
	grep -nF "_('Unable to read source IP.')" "$VIEW_DIR/rules.js"
	grep -nF "renderSummaryRow(_('Source IP')" "$VIEW_DIR/rules.js"
	grep -nF "_('Enable after creation')" "$VIEW_DIR/rules.js"
	grep -nF "this.renderRuleWizard(this.pageData)" "$VIEW_DIR/rules.js"
	grep -nF "this.useRuleEditorLabels(s)" "$VIEW_DIR/rules.js"
	grep -nF "section.renderSectionAdd = function()" "$VIEW_DIR/rules.js"
	grep -nF "return E([])" "$VIEW_DIR/rules.js"
	! grep -nF "this.useRuleCreateFlow(s)" "$VIEW_DIR/rules.js"
	! grep -nF "placeholder: _('New rule name')" "$VIEW_DIR/rules.js"
	grep -nF "uci.add('qddns', 'rule', this.nextNumericSectionId())" "$VIEW_DIR/rules.js"
	grep -nF "uci.set('qddns', sectionId, 'name', ruleName)" "$VIEW_DIR/rules.js"
	grep -nF "uci.set('qddns', sectionId, 'provider', provider)" "$VIEW_DIR/rules.js"
	grep -nF "uci.set('qddns', sectionId, 'source', source)" "$VIEW_DIR/rules.js"
	grep -nF "uci.set('qddns', sectionId, 'record_type', recordType)" "$VIEW_DIR/rules.js"
	grep -nF "uci.set('qddns', sectionId, 'zone', zone)" "$VIEW_DIR/rules.js"
	grep -nF "uci.set('qddns', sectionId, 'record_name', recordName)" "$VIEW_DIR/rules.js"
	! grep -nF "uci.set('qddns', sectionId, 'ttl', '300')" "$VIEW_DIR/rules.js"
	grep -nF "uci.set('qddns', sectionId, 'check_interval', '60')" "$VIEW_DIR/rules.js"
	grep -nF "uci.set('qddns', sectionId, 'force_interval', '3600')" "$VIEW_DIR/rules.js"
	grep -nF "uci.set('qddns', sectionId, 'retry_count', '3')" "$VIEW_DIR/rules.js"
	grep -nF "uci.set('qddns', sectionId, 'retry_backoff', '30')" "$VIEW_DIR/rules.js"
	grep -nF "sourceFamily" "$VIEW_DIR/rules.js"
	grep -nF "wizardSourceFamily" "$VIEW_DIR/rules.js"
	grep -nF "data-probed-family" "$VIEW_DIR/rules.js"
	grep -nF "_('Record type must match the selected source address family.')" "$VIEW_DIR/rules.js"
	grep -nF "return uci.save().then(function()" "$VIEW_DIR/rules.js"
	grep -nF "window.location.reload()" "$VIEW_DIR/rules.js"
	grep -nF "select.appendChild(E('option', { value: choice.id }, [choice.name || emptyText]))" "$VIEW_DIR/rules.js"
	grep -nF "probeSourceDraft: function(source)" "$VIEW_DIR/shared.js"
	grep -nF "probe_source_draft" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF '"probe_source_draft"' "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/acl.d/luci-app-qddns.json"
	python3 - <<'PYEOF'
import os
from pathlib import Path
rules = Path(os.environ['VIEW_DIR'], 'rules.js').read_text()
shared = Path(os.environ['VIEW_DIR'], 'shared.js').read_text()
root = Path(os.environ['VIEW_DIR']).parents[4]
rpcd = Path(root, 'root/usr/share/rpcd/ucode/qddns.uc').read_text()
modal_start = rules.index('showRuleWizardModal: function')
modal_end = rules.index('renderRuleWizard: function')
modal = rules[modal_start:modal_end]
save_start = modal.index('function saveNewSource()')
save_end = modal.index('function updateButtons()', save_start)
save_block = modal[save_start:save_end]
if 'syncWizardRecordType: function(control, family)' not in rules:
    raise SystemExit('rule wizard must sync A/AAAA from the selected source address family')
if "normalized === 'ipv6' ? 'AAAA'" not in rules or "normalized === 'ipv4' ? 'A'" not in rules:
    raise SystemExit('rule wizard record type sync must map ipv6 to AAAA and ipv4 to A')
if 'viewRef.syncWizardRecordType(fields.recordType, source?.family)' not in modal:
    raise SystemExit('rule wizard must sync record type immediately from saved source family')
if 'viewRef.syncWizardRecordType(fields.recordType, sourceProbe.family)' not in modal:
    raise SystemExit('rule wizard must sync record type from probed source IP family')
if "setWizardProbeFeedback(_('Probing source IP...'), 'loading')" not in modal:
    raise SystemExit('rule wizard must put source IP probe loading into the guide feedback')
if "function sourceDetectedMessage(address)" not in modal:
    raise SystemExit('rule wizard must centralize source IP detected guide feedback')
if "function ensureSavedSourceSelected()" not in modal or "ensureSavedSourceSelected();" not in modal:
    raise SystemExit('rule wizard must select and probe the first saved source when switching to saved source mode')
if "The saved source will be used for this rule." not in modal:
    raise SystemExit('rule wizard must not claim saved sources will be saved with the rule')
if "setWizardProbeFeedback(sourceDetectedMessage(result.address), 'ready')" not in modal:
    raise SystemExit('rule wizard must put detected source IP into the guide feedback')
if "setWizardProbeFeedback(message, 'error')" not in modal:
    raise SystemExit('rule wizard must put source IP probe failures into the guide feedback')
if "setWizardSourceIp(_('Not previewable in LuCI'), 'warning')" not in modal:
    raise SystemExit('rule wizard must allow saved non-previewable sources without presenting them as failed probes')
if "const message = qddns.extractResultMessage(err, _('Unable to read source IP.'))" not in modal or "setWizardProbeFeedback(message, 'error')" not in modal:
    raise SystemExit('rule wizard must show saved source XHR/probe errors in the guide feedback')
if "const message = qddns.extractResultMessage(result, _('Unable to read source IP.'))" not in save_block or "setWizardProbeFeedback(message, 'error')" not in save_block:
    raise SystemExit('rule wizard must show backend draft probe errors instead of replacing them with a generic message')
if "qddns.listInterfaces()" not in rules or "catalog.interfaces = qddns.normalizeInterfaces(data[3])" not in rules:
    raise SystemExit('rule wizard must load interfaces for the multi-select source field')
if "function interfaceRank(name)" not in shared or "value.indexOf('pppoe-') === 0" not in shared:
    raise SystemExit('rule wizard interface list must prioritize WAN/PPPoE choices')
if "sourceInterface: this.renderWizardInterfaceSelect(data?.catalog?.interfaces)" not in modal:
    raise SystemExit('rule wizard must render the source interface as a multi-select')
if "control?.multiple" not in rules or "selectedOptions" not in rules:
    raise SystemExit('rule wizard value helper must read multi-selected interfaces')
if "function setSourceInterfaceValue(value)" not in modal:
    raise SystemExit('rule wizard must keep the source interface multi-select helper')
if "setSourceInterfaceValue(lease?.interface" in modal:
    raise SystemExit('rule wizard must not copy LAN host interfaces into the source WAN interface field')
if "lease?.host_interface" not in modal:
    raise SystemExit('rule wizard must display LAN host interfaces separately from source WAN interfaces')
if "nextButton.disabled = stepIndex === 0 && sourceProbe.loading" not in modal:
    raise SystemExit('rule wizard must disable Next while source IP probing is loading')
if "fields.source?.getAttribute('data-source-ip-error') === '1'" not in rules:
    raise SystemExit('rule wizard must block the source step after a failed source IP probe')
for css in [
    "--qddns-rule-wizard-width:min(64rem,94vw);",
    "--qddns-rule-wizard-field-min:18rem;",
    "--qddns-rule-wizard-meta-label:5.5rem;",
    ".modal.qddns-rule-wizard-dialog{align-items:stretch;width:var(--qddns-rule-wizard-width);max-width:94vw}",
    ".modal.qddns-rule-wizard-dialog>h4{box-sizing:border-box;width:100%;margin:0 0 var(--qddns-space-3);padding:0;text-align:left;font-size:1.2rem;font-weight:700;line-height:1.3!important}",
    ".qddns-rule-wizard-modal{box-sizing:border-box;display:grid;align-items:stretch;justify-items:stretch;gap:var(--qddns-space-4);width:100%;max-width:100%;min-width:0;text-align:left;line-height:1.45}",
    ".qddns-rule-wizard-panel{display:grid;justify-items:stretch;gap:var(--qddns-space-3);width:100%;max-width:100%;min-width:0;justify-self:stretch;text-align:left}",
    ".qddns-rule-wizard-panel h4{justify-self:start;margin:0;padding:0;text-align:left;font-size:1.05rem;font-weight:700;line-height:1.35!important}",
    ".qddns-rule-wizard-grid{display:grid;align-items:start;justify-items:stretch;grid-template-columns:repeat(auto-fit,minmax(min(100%,var(--qddns-rule-wizard-field-min)),1fr));gap:var(--qddns-space-3);width:100%;min-width:0}",
    ".qddns-rule-wizard-grid-narrow{grid-template-columns:minmax(0,18rem);max-width:18rem}",
    ".qddns-rule-wizard-field{display:flex;flex-direction:column;gap:var(--qddns-space-1);min-width:0;text-align:left}",
    ".qddns-rule-wizard-field label{font-weight:600;line-height:1.35;text-align:left}",
    ".qddns-rule-wizard-source-panel{display:grid;justify-items:stretch;gap:var(--qddns-space-3);width:100%;min-width:0;text-align:left}",
    ".qddns-rule-wizard-source-actions{align-items:center;justify-content:flex-start}",
    ".qddns-rule-wizard-source-status{display:grid;justify-items:start;gap:var(--qddns-space-1);box-sizing:border-box;width:100%;min-width:0;padding:var(--qddns-space-3);border:1px solid var(--qddns-border);border-radius:var(--qddns-radius-sm);background:var(--qddns-surface);text-align:left}",
    ".qddns-rule-wizard-footer-actions{width:100%;max-width:100%;justify-self:stretch;justify-content:flex-end}",
    ".qddns-rule-wizard-summary-row{display:grid;grid-template-columns:minmax(var(--qddns-rule-wizard-meta-label),max-content) minmax(0,1fr);gap:var(--qddns-space-2);min-width:0;text-align:left}",
]:
    if css not in rules:
        raise SystemExit(f'rule wizard layout must keep fields/cards aligned: missing {css}')
if ".qddns-lease-card{appearance:none;box-sizing:border-box;display:grid;align-items:start;justify-items:stretch;justify-content:stretch;gap:var(--qddns-space-2);width:100%!important;min-width:0;margin:0;padding:var(--qddns-space-3);border:1px solid var(--qddns-border);border-radius:var(--qddns-radius-sm);background:var(--qddns-surface);color:inherit;font:inherit;line-height:1.35;text-align:left!important;text-transform:none;cursor:pointer}" not in shared:
    raise SystemExit('shared lease card layout must keep fields/cards aligned')
if "qddns.renderLeaseCard({" not in modal:
    raise SystemExit('rule wizard must use the shared lease card renderer')
if "qddns-rule-wizard-lease-card" in rules:
    raise SystemExit('rule wizard must not keep a duplicate lease card class')
if "ui.showModal(_('Guided DDNS rule setup'), [modal], 'qddns-rule-wizard-dialog')" not in modal:
    raise SystemExit('rule wizard modal title must use the qddns dialog class for left-aligned layout')
if "E('div', { class: 'qddns-actions qddns-rule-wizard-footer-actions' }" not in modal:
    raise SystemExit('rule wizard modal footer actions must be scoped separately from source actions')
for required in [
    "sourceMode: E('select'",
    "E('option', { value: 'new' }, [_('Create new source')])",
    "E('option', { value: 'saved' }, [_('Use saved source')])",
    "function saveNewSource()",
    "function loadWizardLeases()",
    "qddns.listDhcpv6Leases(mode)",
    "qddns.probeSourceDraft(sourceData)",
    "sourceVersion !== sourceCreate.version",
    "uci.add('qddns', 'source', sectionId)",
    "uci.set('qddns', sectionId, 'type', sourceData.type)",
]:
    if required not in modal:
        raise SystemExit(f'rule wizard must support full source creation flow: missing {required}')
if "fields.source?.getAttribute('data-source-create-dirty') === '1'" not in rules:
    raise SystemExit('rule wizard must block Next when a newly created source has unsaved changes')
if "function buildSourceData()" not in modal:
    raise SystemExit('rule wizard must build new source data by current source type only')
if "function resetSourceTypeFields(sourceType)" not in modal:
    raise SystemExit('rule wizard must reset source-type fields when the source type changes')
reset_block = modal[modal.index('function resetSourceTypeFields(sourceType)'):modal.index('function restoreNewSourceProbe()', modal.index('function resetSourceTypeFields(sourceType)'))]
for required_reset in [
    "fields.sourceFamily.value = isDhcpv6Source ? 'ipv6' : '';",
    "fields.sourceAddress.value = '';",
    "setSourceInterfaceValue('');",
    "fields.sourceDuid.value = '';",
    "fields.sourceIaid.value = '';",
    "fields.sourceMac.value = '';",
    "fields.sourceLeaseFile.value = isDhcpv6Source ? '/tmp/odhcpd.leases' : '';",
    "fields.sourceHostnameHint.value = '';",
    "fields.sourcePrefixFilter.value = '';",
]:
    if required_reset not in reset_block:
        raise SystemExit(f'rule wizard must not retain old source-type state: missing {required_reset}')
if "const sourceOptions = ['family', 'address', 'interface', 'duid', 'iaid', 'mac', 'lease_file', 'hostname_hint', 'prefix_filter'];" not in save_block:
    raise SystemExit('new source wizard must clear options that are hidden by the current source type')
if 'uci.save()' in save_block:
    raise SystemExit('new source wizard must probe a draft source before staging it, not uci.save before probing')
if save_block.index('qddns.probeSourceDraft(sourceData)') > save_block.index("uci.add('qddns', 'source', sectionId)"):
    raise SystemExit('new source wizard must not stage the source until draft probing succeeds')
if 'function restoreNewSourceProbe()' not in modal:
    raise SystemExit('rule wizard must restore a clean draft-probed source without using the saved-source probe path')
if 'sourceCreate.address = result.address' not in save_block or 'sourceCreate.family = probedFamily' not in save_block:
    raise SystemExit('rule wizard must cache the successful draft probe result on the staged new source')
if "result.family || viewRef.inferSourceFamily(result.address" not in save_block:
    raise SystemExit('rule wizard must infer A/AAAA family from the probed address when backend omits family')
if save_block.index('const probedFamily = result.family || viewRef.inferSourceFamily') > save_block.index('sourceOptions.forEach(function(option)'):
    raise SystemExit('new source wizard must persist the probed family before writing UCI source options')
if 'sourceData.family = probedFamily' not in save_block:
    raise SystemExit('new source wizard must store the probed family in the staged source configuration')
clean_start = modal.index('if (sourceCreate.clean && sourceCreate.id)')
clean_end = modal.index('} else {', clean_start)
clean_branch = modal[clean_start:clean_end]
if 'updateWizardSourceProbe()' in clean_branch:
    raise SystemExit('rule wizard must not call saved-source probe for a clean staged source that is not saved yet')
if 'restoreNewSourceProbe()' not in clean_branch:
    raise SystemExit('rule wizard must restore cached draft probe state when returning to a clean staged source')
if "method: 'probe_source_draft'" not in shared:
    raise SystemExit('shared RPC must expose draft source probing')
if 'writefile(draft_probe_config' not in rpcd or "sources probe ${draft_probe_source_id}" not in rpcd:
    raise SystemExit('rpcd must probe draft source through a temporary qddns config')
if "readfile('/etc/config/qddns')" in rpcd or 'writefile(draft_probe_config, source_config)' not in rpcd:
    raise SystemExit('rpcd draft probe must not copy provider secrets into the temporary qddns config')
if '${mktemp_cmd} ${draft_probe_config_template}' not in rpcd or "/tmp/qddns-luci-source-probe.conf" in rpcd:
    raise SystemExit('rpcd draft probe must use a unique temporary config path')
if "const mktemp_cmd = '/bin/mktemp';" not in rpcd:
    raise SystemExit('rpcd draft probe must use an ACL-bound mktemp path')
if "function draft_lease_file(value)" not in rpcd or "let lease_file = draft_lease_file(req.args.lease_file);" not in rpcd:
    raise SystemExit('rpcd draft probe must normalize and restrict draft lease_file input')
if "draft_source_option('lease_file', req.args.lease_file)" in rpcd or "draft_source_option('lease_file', lease_file)" not in rpcd:
    raise SystemExit('rpcd draft probe must not pass arbitrary lease_file paths into qddnsctl')
if '2>&1' not in rpcd or "return { ok: false, error: output || 'command failed' }" not in rpcd:
    raise SystemExit('rpcd must preserve qddnsctl probe error text for LuCI instead of dropping stderr')
if 'list_interfaces' not in rpcd or "ip_cmd} -o link show" not in rpcd:
    raise SystemExit('rpcd must expose interface choices for the rule wizard')
if '(?:' in rpcd:
    raise SystemExit('rpcd ucode must not use unsupported non-capturing regex groups')
if rpcd.index('function push_unique') > rpcd.index('function interface_values'):
    raise SystemExit('rpcd interface_values must not call a later helper; deployed ucode does not hoist it')
source_family = rpcd[rpcd.index('function source_family(section)'):rpcd.index('function section_to_obj(section)')]
if source_family.index("section.type == 'dhcpv6_duid' || section.type == 'dhcpv6_mac'") > source_family.index("family == 'ipv4' || family == 'ipv6'"):
    raise SystemExit('rpcd source_family must force DHCPv6 sources to ipv6 before honoring stale family options')
if "sourceCreate.clean = false;" not in save_block or "setEffectiveSource('', sourceData.name || _('Unnamed source'))" not in save_block:
    raise SystemExit('new source wizard must clear stale draft probe cache after a failed reprobe')
for required_interface_merge in [
	    'interfaces: []',
	    'push_unique(entry.interfaces, fields[1])',
	    'push_unique(entry.interfaces, fields[2])',
	    "entry.host_interface = join(',', entry.interfaces)",
	    'delete entry.interfaces',
	]:
    if required_interface_merge not in rpcd:
        raise SystemExit(f'rpcd DHCPv6 lease candidates must preserve multi-interface choices: missing {required_interface_merge}')
update_source_mode = modal[modal.index('function updateSourceMode()'):modal.index('function renderSummaryRow')]
saved_mode_branch = update_source_mode[update_source_mode.index('} else {'):]
if 'sourceCreate.version++' not in saved_mode_branch:
    raise SystemExit('rule wizard must invalidate pending draft source probes when switching to saved source mode')
failed_result = save_block[save_block.index('if (qddns.isFailedResult(result) || !result.address)'):save_block.index("if (!sourceCreate.id)")]
for required_clear in [
    'sourceProbe.address = \'\';',
    'sourceProbe.family = \'\';',
    "fields.source.removeAttribute('data-probed-family');",
]:
    if required_clear not in failed_result:
        raise SystemExit('new source wizard must clear stale probed source state after a failed draft reprobe')
catch_block = save_block[save_block.index('}).catch(function(err)'):]
for required_clear in [
    'sourceProbe.address = \'\';',
    'sourceProbe.family = \'\';',
    "fields.source.removeAttribute('data-probed-family');",
]:
    if required_clear not in catch_block:
        raise SystemExit('new source wizard must clear stale probed source state after a draft probe exception')
for field in ["this.renderWizardField(_('Record type')", "this.renderWizardField(_('Provider')", "this.renderWizardField(_('Source')", "this.renderWizardField(_('Zone')", "this.renderWizardField(_('Record name')"]:
    if modal.count(field) != 1:
        raise SystemExit(f'{field} must appear exactly once as a field in the modal wizard')
if modal.index("this.renderWizardField(_('Source')") > modal.index("this.renderWizardField(_('Record type')"):
    raise SystemExit('source field must be first in the wizard')
if "fields.source.focus" not in modal:
    raise SystemExit('source field must receive initial focus')
PYEOF
	! grep -nF "this.renderWizardField(_('Rule name'), fields.name)" "$VIEW_DIR/rules.js"
	! grep -nF "name: E('input'" "$VIEW_DIR/rules.js"
	! grep -nF "this.renderWizardField(_('TTL'), fields.ttl)" "$VIEW_DIR/rules.js"
	! grep -nF "this.renderWizardField(_('TTL')" "$VIEW_DIR/rules.js"
	! grep -nF "this.renderWizardField(_('Check interval'), fields.checkInterval)" "$VIEW_DIR/rules.js"
	! grep -nF "this.renderWizardField(_('Force interval'), fields.forceInterval)" "$VIEW_DIR/rules.js"
	! grep -nF "this.renderWizardField(_('Retry backoff'), fields.retryBackoff)" "$VIEW_DIR/rules.js"
	! grep -nF "choice.id +" "$VIEW_DIR/rules.js"
	! grep -nF "provider.id +" "$VIEW_DIR/rules.js"
	! grep -nF "source.id +" "$VIEW_DIR/rules.js"
}

check_source_probe_no_luci_rpc_recursion() {
	! grep -nF '"luci-rpc", "getHostHints"' "$ROOT_DIR/qddns/src/source.rs"
	! grep -nF "collect_host_hint_ipv6_candidates(&normalized_mac, &mut matches);" "$ROOT_DIR/qddns/src/source.rs"
	! grep -nF "ubus call luci-rpc getHostHints" "$ROOT_DIR/qddns/src/source.rs"
}

check_source_only_draft_probe() {
	{
		printf "config source 'wizard_probe'\n"
		printf "\toption name 'Wizard source'\n"
		printf "\toption type 'local_addr'\n"
		printf "\toption family 'ipv4'\n"
		printf "\toption address '192.0.2.10'\n"
	} > "$SELFTEST_DRAFT_CONFIG"

	cargo run --quiet --bin qddnsctl -- --config "$SELFTEST_DRAFT_CONFIG" sources probe wizard_probe |
		grep -q '"address":"192.0.2.10"'
	cargo run --quiet --bin qddnsctl -- --config "$SELFTEST_DRAFT_CONFIG" sources probe wizard_probe |
		grep -q '"source":"wizard_probe"'
}

check_settings_boundary() {
	grep -nF "form.NamedSection, 'main'" "$VIEW_DIR/settings.js"
	grep -nF "form.GridSection, 'source'" "$VIEW_DIR/settings.js"
	grep -nF "form.GridSection, 'provider'" "$VIEW_DIR/settings.js"
	grep -nF 'qddns.probeSource' "$VIEW_DIR/settings.js"
	! grep -nE 'qddns\.(runRule|testRule|getRuleStatus)|form.GridSection, .rule.' "$VIEW_DIR/settings.js"
}

check_settings_source_probe_previewability() {
	grep -nF "isProbeableSourceType: function(sourceType)" "$VIEW_DIR/shared.js"
	grep -nF "if (!qddns.isProbeableSourceType(src.type))" "$VIEW_DIR/settings.js"
	grep -nF "qddns.renderBadge(_('Not previewable in LuCI'), 'warning')" "$VIEW_DIR/settings.js"
	grep -nF "probeButton.disabled = true;" "$VIEW_DIR/settings.js"
	grep -nF "probeButton.disabled = !qddns.isProbeableSourceType(this.getSourceType(sectionId, optionSet));" "$VIEW_DIR/settings.js"
	! grep -nF "isProbeableSourceType: function(sourceType)" "$VIEW_DIR/rules.js" "$VIEW_DIR/settings.js"
}

check_name_visible_numeric_hidden_ui() {
	grep -nF "require uci" "$VIEW_DIR/settings.js"
	grep -nF "require ui" "$VIEW_DIR/settings.js"
	grep -nF "require tools.widgets as widgets" "$VIEW_DIR/settings.js"
	grep -nF "require uci" "$VIEW_DIR/rules.js"
	grep -nF "require ui" "$VIEW_DIR/rules.js"
	grep -nF "nextNumericSectionId" "$VIEW_DIR/settings.js"
	grep -nF "nextNumericSectionId" "$VIEW_DIR/rules.js"
	grep -nF "uci.sections('qddns')" "$VIEW_DIR/settings.js"
	grep -nF "uci.sections('qddns')" "$VIEW_DIR/rules.js"
	grep -nF "for (let index = 1; true; index++)" "$VIEW_DIR/settings.js"
	grep -nF "for (let index = 1; true; index++)" "$VIEW_DIR/rules.js"
	grep -nF "uci.add('qddns', 'provider', this.nextNumericSectionId())" "$VIEW_DIR/settings.js"
	grep -nF "uci.set('qddns', sectionId, 'name', providerName)" "$VIEW_DIR/settings.js"
	grep -nF "const sectionId = viewRef.nextNumericSectionId()" "$VIEW_DIR/settings.js"
	grep -nF "const addTask = handleAdd.call(this, ev, sectionId)" "$VIEW_DIR/settings.js"
	grep -nF "uci.set(configName, sectionId, 'name', visibleName)" "$VIEW_DIR/settings.js"
	grep -nF "uci.set('qddns', sectionId, 'name', ruleName)" "$VIEW_DIR/rules.js"
	grep -nF "useNameCreateFlow" "$VIEW_DIR/settings.js"
	test "$(grep -cF "this.useNameCreateFlow(s," "$VIEW_DIR/settings.js")" -eq 2
	grep -nF "useRuleEditorLabels" "$VIEW_DIR/rules.js"
	grep -nF "section.sectiontitle = function(sectionId)" "$VIEW_DIR/settings.js"
	grep -nF "section.sectiontitle = function(sectionId)" "$VIEW_DIR/rules.js"
	grep -nF "return uci.get('qddns', sectionId, 'name') || options.unnamed" "$VIEW_DIR/settings.js"
	grep -nF "return uci.get('qddns', sectionId, 'name') || _('Unnamed rule')" "$VIEW_DIR/rules.js"
	grep -nF "nameHeader.textContent = _('Name')" "$VIEW_DIR/settings.js"
	grep -nF "nameHeader.textContent = _('Name')" "$VIEW_DIR/rules.js"
	grep -nF "_('New source name')" "$VIEW_DIR/settings.js"
	grep -nF "_('New provider name')" "$VIEW_DIR/settings.js"
	! grep -nF "_('New rule name')" "$VIEW_DIR/rules.js"
	grep -nF "_('Provider name')" "$VIEW_DIR/settings.js"
	grep -nF "_('Source name must not be empty.')" "$VIEW_DIR/settings.js"
	grep -nF "_('Provider name must not be empty.')" "$VIEW_DIR/settings.js"
	! grep -nF "_('Rule name must not be empty.')" "$VIEW_DIR/rules.js"
	grep -nF "_('Unnamed source')" "$VIEW_DIR/settings.js"
	grep -nF "_('Unnamed provider')" "$VIEW_DIR/settings.js"
	grep -nF "_('Unnamed rule')" "$VIEW_DIR/rules.js"
	grep -nF "s.option(form.Value, 'name', _('Name')" "$VIEW_DIR/settings.js"
	test "$(grep -cF "s.option(form.Value, 'name', _('Name')" "$VIEW_DIR/settings.js")" -eq 2
	grep -nF "s.option(form.Value, 'name', _('Name')" "$VIEW_DIR/rules.js"
	grep -nF "o.placeholder = _('Unnamed source')" "$VIEW_DIR/settings.js"
	grep -nF "o.placeholder = _('Unnamed provider')" "$VIEW_DIR/settings.js"
	! grep -nF "o.readonly = true;" "$VIEW_DIR/settings.js"
	grep -nF "o = s.option(widgets.DeviceSelect, 'interface', _('WAN/upstream interface')" "$VIEW_DIR/settings.js"
	python3 - <<'PYEOF'
import os
from pathlib import Path

settings = Path(os.environ['VIEW_DIR'], 'settings.js').read_text().splitlines()
source_start = next((i for i, line in enumerate(settings) if "s = m.section(form.GridSection, 'source'" in line), None)
provider_start = next((i for i, line in enumerate(settings) if "s = m.section(form.GridSection, 'provider'" in line), None)
if source_start is None or provider_start is None:
    raise SystemExit('source/provider GridSection blocks are missing')
source_block = '\n'.join(settings[source_start:provider_start])
if 's.nodescriptions = true;' not in source_block:
    raise SystemExit('source GridSection must suppress option description rows; they break named table alignment')
start = next((i for i, line in enumerate(settings) if "o = s.option(widgets.DeviceSelect, 'interface', _('WAN/upstream interface')" in line), None)
if start is None:
    raise SystemExit('source interface DeviceSelect option is missing')
end = next((i for i in range(start + 1, len(settings)) if '\to = s.option(' in settings[i]), len(settings))
block = '\n'.join(settings[start:end])
if "For DHCPv6 DUID/MAC sources, choose WAN/upstream interface(s); DHCPv6-PD route source prefixes from those interfaces validate LAN host IPv6 addresses." not in block:
    raise SystemExit('source interface modal guidance is missing')
if 'o.multiple = true;' not in block:
    raise SystemExit('source interface selector must enable multi-select')
if 'o.multiple = false;' in block:
    raise SystemExit('source interface selector must not force single-select')
if 'viewRef.interfaceValues(value)' not in block:
    raise SystemExit('source interface selector must normalize comma and UCI list values for settings round-trip')
if "const stored = arguments.length > 1 ? value : (this.data?.[sectionId] || uci.get('qddns', sectionId, 'interface'));" not in block:
    raise SystemExit('source interface selector must read saved UCI interface values when rendering existing sources')
if "const interfaceWrite = o.write;" not in block or "interfaceWrite.call(this, sectionId, viewRef.interfaceValues(value))" not in block:
    raise SystemExit('source interface selector must write multi-select values as normalized UCI lists')
if 'const SOURCE_OPTION_FIELDS' not in '\n'.join(settings) or 'cleanupSourceTypeOptions' not in source_block:
    raise SystemExit('settings source type changes must clear stale options from the previous source type')
if "dhcpv6_duid: ['interface', 'duid', 'iaid', 'lease_file', 'hostname_hint', 'prefix_filter']" not in '\n'.join(settings):
    raise SystemExit('settings DHCPv6 DUID sources must not keep stale family/address/mac fields')
if "dhcpv6_mac: ['interface', 'mac', 'lease_file', 'hostname_hint', 'prefix_filter']" not in '\n'.join(settings):
    raise SystemExit('settings DHCPv6 MAC sources must not keep stale family/address/duid fields')
if 'guardSourceOptionWrite' not in source_block:
    raise SystemExit('settings source fields must refuse writes while hidden by the current source type')
PYEOF
	grep -nF "o.noaliases = true;" "$VIEW_DIR/settings.js"
	grep -nF "o.nocreate = true;" "$VIEW_DIR/settings.js"
	grep -nF "o.placeholder = _('Unnamed rule')" "$VIEW_DIR/rules.js"
	grep -nF "o = s.option(form.ListValue, 'provider', _('Provider'))" "$VIEW_DIR/rules.js"
	grep -nF "o = s.option(form.ListValue, 'source', _('Source'))" "$VIEW_DIR/rules.js"
	grep -nF "o.value(provider.id, provider.name || _('Unnamed provider'))" "$VIEW_DIR/rules.js"
	grep -nF "o.value(source.id, source.name || _('Unnamed source'))" "$VIEW_DIR/rules.js"
	grep -nF "this.getRuleLabel(rule)" "$VIEW_DIR/rules.js"
	grep -nF "this.getSourceLabel(rule.source)" "$VIEW_DIR/rules.js"
	grep -nF "this.getProviderLabel(rule.provider)" "$VIEW_DIR/rules.js"
	grep -nF "choices.push({ value: rule.id, label: _('Rule') + ': ' + this.getRuleLabel(rule) })" "$VIEW_DIR/logs.js"
	grep -nF "name: section.name || null" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "this.ruleLabel(item.id)" "$VIEW_DIR/overview.js"
	grep -nF "getScopeLabel" "$VIEW_DIR/logs.js"
	grep -nF "this.getScopeLabel(entry.scope || 'system')" "$VIEW_DIR/logs.js"
	grep -nF "this.getScopeLabel(entry?.scope || 'system')" "$VIEW_DIR/logs.js"
	grep -nF "formatLogLine" "$VIEW_DIR/logs.js"
	grep -nF "_('Log Output')" "$VIEW_DIR/logs.js"
	! grep -nF "entry.scope || '-'" "$VIEW_DIR/logs.js"
	! grep -nF "this.logsData?.content" "$VIEW_DIR/logs.js"
	! grep -nF "_('Raw Log Output')" "$VIEW_DIR/logs.js"
	grep -nF "const PROVIDER_TEMPLATES" "$VIEW_DIR/settings.js"
	grep -nF "cloudflare" "$VIEW_DIR/settings.js"
	grep -nF "dnspod" "$VIEW_DIR/settings.js"
	grep -nF "aliyun" "$VIEW_DIR/settings.js"
	grep -nF "custom_http" "$VIEW_DIR/settings.js"
	grep -nF "uci.save()" "$VIEW_DIR/settings.js"
	grep -nF "type: 'custom_http'" "$VIEW_DIR/settings.js"
	grep -nF "method: 'POST'" "$VIEW_DIR/settings.js"
	grep -nF '"record":"{{record_name}}"' "$VIEW_DIR/settings.js"
	! grep -nF '"record":"{{record}}"' "$VIEW_DIR/settings.js"
	grep -nF "success_contains: 'ok'" "$VIEW_DIR/settings.js"
	! grep -nE "Provider ID|Source ID|Rule ID|Section ID|New Source ID|New Provider ID|Display name|Unset display name|section ID|internal numeric|internal reference|stable reference key|stable key|stable Source ID|stable Provider ID" "$VIEW_DIR/settings.js" "$VIEW_DIR/rules.js" "$VIEW_DIR/logs.js"
	! grep -nF "_('ID')" "$VIEW_DIR/settings.js" "$VIEW_DIR/rules.js" "$VIEW_DIR/logs.js"
	! grep -nE "Provider ID|Source ID|Rule ID|Section ID|New Source ID|New Provider ID|Display name|Unset display name|section ID|internal numeric|internal reference|stable reference key|stable key|stable Source ID|stable Provider ID" "$PO_FILE"
	! grep -nF 'msgid "ID"' "$PO_FILE"
	! grep -nF "|| sectionId" "$VIEW_DIR/settings.js" "$VIEW_DIR/rules.js"
	! grep -nF "provider.name || provider.id" "$VIEW_DIR/rules.js"
	! grep -nF "source.name || source.id" "$VIEW_DIR/rules.js"
	! grep -nF "rule.id," "$VIEW_DIR/rules.js"
	! grep -nF "rule.id || '-'" "$VIEW_DIR/logs.js"
	! grep -nF "uci.add('qddns', 'provider', providerId)" "$VIEW_DIR/settings.js"
	! grep -nF "validateProviderId" "$VIEW_DIR/settings.js"
	! grep -nF "useSectionIdColumnHeader" "$VIEW_DIR/settings.js"
	! grep -nF "useCreateIdHint" "$VIEW_DIR/settings.js"
	! grep -nF "s.option(form.DummyValue, '_display_name'" "$VIEW_DIR/settings.js"
	! grep -nF "s.option(form.DummyValue, '_section_id'" "$VIEW_DIR/settings.js" "$VIEW_DIR/rules.js"
	! grep -nE "can be edited later|renaming referenced sections|changing visible names|rule sections|source sections|provider sections|referenced sections|provider section" "$VIEW_DIR/settings.js" "$VIEW_DIR/rules.js"
}

check_dhcpv6_lease_fill_ui() {
	grep -nF "const callDhcpv6Leases = rpc.declare({ object: 'qddns', method: 'list_dhcpv6_leases', params: ['mode'], expect: {} });" "$VIEW_DIR/shared.js"
	grep -nF "listDhcpv6Leases: callDhcpv6Leases" "$VIEW_DIR/shared.js"
	grep -nF "handleDhcpv6LeaseLoad" "$VIEW_DIR/settings.js"
	grep -nF "fillDhcpv6Lease" "$VIEW_DIR/settings.js"
	grep -nF "setSourceOptionValue" "$VIEW_DIR/settings.js"
	grep -nF "renderDhcpv6LeaseStatus" "$VIEW_DIR/settings.js"
	grep -nF "qddns.listDhcpv6Leases(this.getDhcpv6LeaseMode(sectionId, optionSet))" "$VIEW_DIR/settings.js"
	grep -nF "getDhcpv6LeaseMode" "$VIEW_DIR/settings.js"
	grep -nF "s.option(form.DummyValue, '_dhcpv6_status', _('Status'))" "$VIEW_DIR/settings.js"
	grep -nF "_('Read current DUID')" "$VIEW_DIR/settings.js"
	grep -nF "_('Read current MAC')" "$VIEW_DIR/settings.js"
	grep -nF "_('Read current DHCPv6 lease candidates, then choose one to fill the DUID source fields.')" "$VIEW_DIR/settings.js"
	grep -nF "_('Read current LAN host candidates, then choose one to fill the MAC source fields.')" "$VIEW_DIR/settings.js"
	grep -nF "_('Fill from this lease')" "$VIEW_DIR/settings.js"
	grep -nF "_('No DHCPv6 leases found.')" "$VIEW_DIR/settings.js"
	grep -nF "_('No LAN hosts with public IPv6 found.')" "$VIEW_DIR/settings.js"
		grep -nF "_('Selected DHCPv6 lease values have been filled. Keep the WAN interface selected separately.')" "$VIEW_DIR/settings.js"
		grep -nF "_('Selected LAN host MAC has been filled. Keep the WAN interface selected separately.')" "$VIEW_DIR/settings.js"
	grep -nF "options.duid" "$VIEW_DIR/settings.js"
	grep -nF "options.mac" "$VIEW_DIR/settings.js"
	grep -nF "options.iaid" "$VIEW_DIR/settings.js"
	grep -nF "options.leaseFile" "$VIEW_DIR/settings.js"
	grep -nF "options.hostnameHint" "$VIEW_DIR/settings.js"
	grep -nF "options.prefixFilter" "$VIEW_DIR/settings.js"
	grep -nF "this.setSourceOptionValue(options.prefixFilter, sectionId, '')" "$VIEW_DIR/settings.js"
	grep -nF "options.interface" "$VIEW_DIR/settings.js"
	grep -nF "const widget = option.getUIElement(sectionId)" "$VIEW_DIR/settings.js"
	grep -nF "widget.setValue(normalized)" "$VIEW_DIR/settings.js"
	grep -nF "widget.node.setAttribute('data-changed', 'true')" "$VIEW_DIR/settings.js"
	grep -nF "widget.node.dispatchEvent(new CustomEvent('widget-change', { bubbles: true }))" "$VIEW_DIR/settings.js"
	grep -nF "getDhcpv6OptionSet" "$VIEW_DIR/settings.js"
	grep -nF "filterDhcpv6Choices" "$VIEW_DIR/settings.js"
	grep -nF "qddns.renderLeaseCard({" "$VIEW_DIR/settings.js"
	grep -nF ".qddns-lease-card{appearance:none;box-sizing:border-box;display:grid;align-items:start;justify-items:stretch;justify-content:stretch;gap:var(--qddns-space-2);width:100%!important;min-width:0;margin:0;padding:var(--qddns-space-3);border:1px solid var(--qddns-border);border-radius:var(--qddns-radius-sm);background:var(--qddns-surface);color:inherit;font:inherit;line-height:1.35;text-align:left!important;text-transform:none;cursor:pointer}" "$VIEW_DIR/shared.js"
	grep -nF "grid-template-columns:1fr" "$VIEW_DIR/shared.js"
	grep -nF "overflow-wrap:anywhere;word-break:normal;white-space:pre-wrap;text-align:left" "$VIEW_DIR/shared.js"
	grep -nF "justify-items:stretch" "$VIEW_DIR/shared.js"
	grep -nF "width:100%;justify-self:stretch" "$VIEW_DIR/shared.js"
	! grep -nF -- "--qddns-dhcpv6-card-min" "$VIEW_DIR/settings.js"
	! grep -nF -- "--qddns-dhcpv6-card-min:10rem;" "$VIEW_DIR/settings.js"
	! grep -nF "grid-template-columns:repeat(auto-fit,minmax(var(--qddns-dhcpv6-card-min),1fr))" "$VIEW_DIR/settings.js"
	grep -nF "this.setSourceOptionValue(options.duid, sectionId, lease?.duid || '')" "$VIEW_DIR/settings.js"
	grep -nF "this.setSourceOptionValue(options.mac, sectionId, lease?.mac || '')" "$VIEW_DIR/settings.js"
		grep -nF "_('Host interface')" "$VIEW_DIR/settings.js"
		grep -nF "lease?.host_interface || '-'" "$VIEW_DIR/settings.js"
		! grep -nF "this.setSourceOptionValue(options.interface, sectionId, lease?.interface || '')" "$VIEW_DIR/settings.js"
	grep -nF "const identityMeta = isDuidSource ? [" "$VIEW_DIR/settings.js"
	grep -nF "_('LAN IP')" "$VIEW_DIR/settings.js"
	grep -nF "_('Prefix narrowing')" "$VIEW_DIR/settings.js"
	! grep -nF "getLeasePrefixFilter" "$VIEW_DIR/settings.js"
	! grep -nF "firstHextet" "$VIEW_DIR/settings.js"
	grep -nF "o.value('dhcpv6_mac', _('MAC'))" "$VIEW_DIR/settings.js"
	grep -nF "o = s.option(form.Value, 'mac', _('MAC'))" "$VIEW_DIR/settings.js"
	grep -nF "this.sourceDhcpv6Options.type = o" "$VIEW_DIR/settings.js"
	grep -nF "renderSourceIpStatus" "$VIEW_DIR/settings.js"
	grep -nF "sourceIpProbe.token++" "$VIEW_DIR/settings.js"
	grep -nF "if (token !== sourceIpProbe.token)" "$VIEW_DIR/settings.js"
	grep -nF "bindSourceOptionChange" "$VIEW_DIR/settings.js"
	python3 - <<'PYEOF'
import os
from pathlib import Path

settings = Path(os.environ['VIEW_DIR'], 'settings.js').read_text()

def block_between(start_marker, end_marker):
    start = settings.index(start_marker)
    end = settings.index(end_marker, start)
    return settings[start:end]

getter = block_between('getSourceOptionValue: function(option, sectionId)', 'getSourceType: function(sectionId, optionSet)')
if "!option.map?.root" not in getter:
    raise SystemExit('getSourceOptionValue must not query UI widgets before the LuCI map root exists')
if getter.index('!option.map?.root') > getter.index('option.getUIElement(sectionId)'):
    raise SystemExit('getSourceOptionValue must guard map.root before getUIElement()')

source_ip_cfg = block_between("o = s.option(form.DummyValue, '_source_ip', _('Source IP'))", "o = s.option(form.DummyValue, '_dhcpv6_status', _('Status'))")
dhcpv6_cfg = block_between("o = s.option(form.DummyValue, '_dhcpv6_status', _('Status'))", "o = s.option(form.Value, 'duid', _('DUID'))")
for name, block in [('_source_ip', source_ip_cfg), ('_dhcpv6_status', dhcpv6_cfg)]:
    if 'if (arguments.length > 1)' not in block:
        raise SystemExit(f'{name} cfgvalue must ignore LuCI load-phase setter calls')
PYEOF
	grep -nF "s.option(form.DummyValue, '_source_ip', _('Source IP'))" "$VIEW_DIR/settings.js"
	grep -nF "o = s.option(form.Value, 'address', _('Address')); o.modalonly = true; o.depends('type', 'local_addr')" "$VIEW_DIR/settings.js"
	grep -nF "_('Save and reload to read updated source IP.')" "$VIEW_DIR/settings.js"
	grep -nF "_('Unable to read source IP.')" "$VIEW_DIR/settings.js"
	! grep -nF "o.value('dhcpv6_mac', _('DHCPv6 MAC'))" "$VIEW_DIR/settings.js"
	! grep -nF "_('Read current DHCPv6 lease candidates, then choose one to fill the MAC source fields.')" "$VIEW_DIR/settings.js"
	grep -nF "input.dispatchEvent(new Event('input', { bubbles: true }))" "$VIEW_DIR/settings.js"
	grep -nF "input.dispatchEvent(new Event('change', { bubbles: true }))" "$VIEW_DIR/settings.js"
	! grep -nF "qddns.renderTableSection(_('DHCPv6 leases')" "$VIEW_DIR/settings.js"
	! grep -nF "s.option(form.Button, '_dhcpv6_leases'" "$VIEW_DIR/settings.js"
	! grep -nF "querySelector('[id=\"%s\"]'" "$VIEW_DIR/settings.js"
	! grep -nF "read_direct('/tmp/odhcpd.leases" "$VIEW_DIR/settings.js" "$VIEW_DIR/shared.js"
}

check_dhcpv6_lease_fill_backend() {
		grep -nF "import { open, popen, stat, writefile, unlink } from 'fs';" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
		! grep -nF "import { connect } from 'ubus';" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
		grep -nF "function source_family(section)" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
		grep -nF "family: source_family(section)" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
		grep -nF "section.type == 'dhcpv6_duid' || section.type == 'dhcpv6_mac'" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "const dhcpv4_lease_file = '/tmp/dhcp.leases';" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "const dhcpv6_lease_file = '/tmp/odhcpd.leases';" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "const dhcpv6_lease_max_bytes = 262144;" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "const dhcpv6_lease_max_entries = 64;" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "const dhcpv6_lease_max_prefixes = 8;" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "function list_dhcpv6_leases(mode)" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "function is_public_ipv6(address)" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "let first = substr(address || '', 0, 1);" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "(first == '2' || first == '3')" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
		! grep -nF "substr(address, 0, 2) == '2'" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
		! grep -nF "substr(address, 0, 2) == '3'" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
		grep -nF "function read_limited_regular_file(path, max_bytes)" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
		grep -nF "info.type != 'file'" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
		grep -nF "file.read(max_bytes + 1)" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
		grep -nF "function add_dhcpv4_lease_entries(entries)" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
		grep -nF "read_limited_regular_file(dhcpv4_lease_file, dhcpv6_lease_max_bytes)" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
		grep -nF "function add_ipv4_neighbor_entries(entries)" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF -- "-4 neigh show" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "push_unique(entry.ipv4, address)" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "add_ipv4_neighbor_entries(entries)" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "function is_private_ipv4(address)" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "substr(address, 0, 3) == '10.'" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "substr(address, 0, 4) == '172.'" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "substr(address, 0, 8) == '192.168.'" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	! grep -nF "ubus.call('luci-rpc', 'getHostHints')" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	! grep -nF "function dhcpv6_prefix_filter" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	! grep -nF "split(prefixes[0]" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	! grep -nF "entry.prefix_filter =" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "function add_ndp_entries(entries)" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "const ip_cmd = '/sbin/ip';" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF -- "-6 neigh show" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "push_unique(entry.prefixes" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "function dhcpv6_duid_mac(duid)" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "let mac = dhcpv6_duid_mac(fields[2]);" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "source_type == 'dhcpv6_mac'" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
		grep -nF "return substr(content, 0, max_bytes)" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
		grep -nF "length(keys(entries)) >= dhcpv6_lease_max_entries" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
		grep -nF "length(prefixes) >= dhcpv6_lease_max_prefixes" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
		grep -nF "read_limited_regular_file(dhcpv6_lease_file, dhcpv6_lease_max_bytes)" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "list_dhcpv6_leases: {" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "args: { mode: 'mode' }" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "return list_dhcpv6_leases(req.args.mode || 'duid');" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "if (mode == 'mac')" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "delete entry.duid;" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "delete entry.iaid;" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "delete entry.lease_file;" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF '"list_dhcpv6_leases"' "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/acl.d/luci-app-qddns.json"
	! grep -nF '"luci-rpc": [ "getHostHints" ]' "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/acl.d/luci-app-qddns.json"
	grep -nF '"/tmp/dhcp.leases": [ "read" ]' "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/acl.d/luci-app-qddns.json"
	grep -nF '"/tmp/odhcpd.leases": [ "read" ]' "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/acl.d/luci-app-qddns.json"
	grep -nF '"/sbin/ip": [ "exec" ]' "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/acl.d/luci-app-qddns.json"
	! grep -nF "req.args.lease" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	! grep -nF "req.args.path" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
}

check_ipv6_prefix_source_guard() {
	python3 - <<PYEOF
from pathlib import Path

source = Path('$ROOT_DIR/qddns/src/source.rs').read_text()
config = Path('$ROOT_DIR/qddns/src/config.rs').read_text()
rpcd = Path('$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc').read_text()
settings = Path('$VIEW_DIR/settings.js').read_text()
rules = Path('$VIEW_DIR/rules.js').read_text()
for bad in [
    'addr.to_string().starts_with',
    '.to_string().starts_with(prefix)',
    'split(prefixes[0]',
    'prefix_filter required',
    'wan_interface',
    'valid_prefix',
    'mac_ipv6_filter',
]:
    haystack = '\\n'.join([source, config, rpcd, settings])
    if bad in haystack:
        raise SystemExit(f'forbidden IPv6 prefix path remains: {bad}')
if 'prefix_len' not in source and 'prefix_length' not in source:
    raise SystemExit('source.rs must use parsed IPv6 prefix length')
if 'wan_source_prefix_selects_first_matching_candidate_without_prefix_filter' not in source:
    raise SystemExit('source.rs must test automatic selection when multiple candidates match a WAN source prefix')
if 'ip -6 route show table all' not in source or 'parse_interface_route_source_ipv6_prefixes' not in source:
    raise SystemExit('source.rs must include WAN route source prefixes for delegated IPv6 PD matching')
if 'wan_route_from_prefix_accepts_delegated_pd_candidate' not in source:
    raise SystemExit('source.rs must test WAN route source prefixes for delegated IPv6 PD matching')
if 'for prefix in interface_wan_source_ipv6_prefixes(iface)?' not in source or 'selected WAN/upstream source prefix set' not in source:
    raise SystemExit('source.rs must merge multi-selected WAN source prefixes and only fail when all selected interfaces lack public route source prefixes')
if "lease?.interface" in settings or "lease?.interface" in rules:
    raise SystemExit('lease cards must not write or display LAN host interfaces as source interface values')
if 'host_interface' not in rpcd or 'host_interface' not in settings or 'host_interface' not in rules:
    raise SystemExit('lease cards must expose LAN host interfaces separately from source WAN interfaces')
if 'fn command_output_with_timeout' not in source or 'SOURCE_COMMAND_TIMEOUT' not in source:
    raise SystemExit('source.rs must bound source subprocess execution')
if 'source_command_output_times_out_slow_commands' not in source:
    raise SystemExit('source.rs must test source subprocess timeout behavior')
if 'fn read_dhcpv6_lease_file' not in source or 'DHCPV6_LEASE_MAX_BYTES' not in source:
    raise SystemExit('source.rs must bound DHCPv6 lease file reads')
if 'fs::canonicalize(path)' not in source:
    raise SystemExit('source.rs must resolve lease_file symlinks before pseudo path checks')
if 'fs::read_to_string(lease_file).unwrap_or_default()' in source:
    raise SystemExit('MAC source must not hide DHCPv6 lease file read errors')
if 'Err(err) if explicit_lease_file.is_some() => return Err(err)' not in source:
    raise SystemExit('MAC source must reject explicit DHCPv6 lease_file errors before NDP fallback')
if 'let opened_metadata = file' not in source or '.metadata()' not in source or 'opened_metadata.file_type().is_file()' not in source:
    raise SystemExit('source.rs must verify DHCPv6 lease file type after opening the file handle')
for blocked in ['path.starts_with("/dev")', 'path.starts_with("/proc")', 'path.starts_with("/sys")']:
    if blocked not in source:
        raise SystemExit(f'source.rs must reject blocking pseudo lease paths: {blocked}')
PYEOF
}

check_default_config_numeric_sections() {
	python3 - <<PYEOF
from pathlib import Path
import re

config = Path('$DEFAULT_CONFIG_FILE').read_text().splitlines()
sections = {}
for line in config:
    m = re.match(r"config (source|provider|rule) '([^']+)'", line)
    if m:
        kind, name = m.groups()
        if not name.isdigit():
            raise SystemExit(f'{kind} section must use numeric ID, got {name}')
        sections[name] = kind

for line in config:
    m = re.match(r"\s*option (provider|source) '([^']+)'", line)
    if m and not m.group(2).isdigit():
        raise SystemExit(f'{m.group(1)} reference must use numeric ID, got {m.group(2)}')
PYEOF
}

check_default_config_dhcpv6_interface() {
	python3 - <<PYEOF
from pathlib import Path

config = Path('$DEFAULT_CONFIG_FILE').read_text()
duid = config[config.index("option type 'dhcpv6_duid'"):]
mac = config[config.index("option type 'dhcpv6_mac'"):]
if "option interface" not in duid.split('\\n\\n', 1)[0]:
    raise SystemExit('dhcpv6_duid sample must set interface')
if "option interface" not in mac.split('\\n\\n', 1)[0]:
    raise SystemExit('dhcpv6_mac sample must set interface')
PYEOF
}

check_name_visible_numeric_hidden_po() {
	for msgid in \
		'Name' \
		'Unnamed source' \
		'Unnamed provider' \
		'Unnamed rule' \
		'New source name' \
		'New provider name' \
		'New rule name' \
		'Provider name' \
		'Source name must not be empty.' \
		'Provider name must not be empty.' \
		'Rule name' \
		'Provider templates' \
		'Create from template' \
		'Custom HTTP' \
		'Copy template' \
		'Template' \
		'Guided DDNS rule setup' \
		'Start guided setup' \
		'Start a short wizard that creates a complete rule with safe defaults. Use the advanced table below for later edits.' \
		'1. Source' \
		'2. DNS' \
		'3. Confirm' \
		'Choose Source IP' \
		'Choose where to update DNS' \
		'Confirm and create the rule' \
		'Rule name is generated automatically from the record.' \
		'Back' \
		'Next' \
		'Add DDNS rule' \
		'No providers available' \
		'No sources available' \
		'Choose the source IP first, then choose the DNS location.' \
		'Source setup' \
		'Create new source' \
		'Use saved source' \
		'Source name' \
		'Source type' \
		'Auto' \
		'IPv4' \
		'IPv6' \
		'Source name is required.' \
		'Address is required.' \
		'WAN/upstream interface' \
		'WAN/upstream interface is required.' \
		'Choose a lease candidate or enter the source values manually.' \
		'Probe source IP' \
		'Probe source IP before continuing. The source will be saved with the rule.' \
		'Create or choose a source before continuing.' \
			'Source IP' \
			'Probing source IP...' \
			'Source IP detected: %s. The source will be saved with the rule.' \
			'Source IP detected: %s. The saved source will be used for this rule.' \
			'Unable to read source IP. Choose another source or fix the source configuration.' \
			'Not previewable in LuCI' \
		'This source type cannot be previewed in LuCI. Confirm the record type manually; the backend will validate it when the rule runs.' \
		'Loading...' \
		'Source IP is still loading.' \
		'Unable to read source IP.' \
		'Save and reload to read updated source IP.' \
		'Provider, source, zone, and record name are required.' \
		'Record type must match the selected source address family.' \
		'Enable after creation' \
		'Saving rule...' \
		'Rule has been staged. Reloading rules page...' \
		'Unable to add the DDNS rule.' \
		'Source is required.' \
		'Provider, zone, and record name are required.' \
		'Local address' \
		'DHCPv6 DUID' \
		'Public probe' \
		'Script' \
		'Status' \
		'Read current DUID' \
		'Read current MAC' \
		'Read current DHCPv6 lease candidates, then choose one to fill the DUID source fields.' \
		'Read current LAN host candidates, then choose one to fill the MAC source fields.' \
			'Choose a current DUID to fill DUID, IAID, and hostname hint. Keep the WAN interface selected separately.' \
			'Choose a current MAC to fill MAC, LAN IP identity, and hostname hint. Keep the WAN interface selected separately.' \
			'Fill from this lease' \
			'No DHCPv6 leases found.' \
			'No LAN hosts with public IPv6 found.' \
			'Selected DHCPv6 lease values have been filled. Keep the WAN interface selected separately.' \
			'Selected LAN host MAC has been filled. Keep the WAN interface selected separately.' \
			'DHCPv6 leases' \
			'LAN hosts' \
			'Unable to load host candidates.' \
			'Unnamed host' \
			'Hostname' \
			'LAN IP' \
			'Host interface' \
			'Prefix' \
			'Prefix narrowing' \
			'Advanced narrowing after WAN/PD source prefix matching; it cannot replace the interface.' \
			'For DHCPv6 DUID/MAC sources, choose WAN/upstream interface(s). QDDNS uses DHCPv6-PD route source prefixes from them; lease cards only fill the LAN host identity.' \
			'For DHCPv6 DUID/MAC sources, choose WAN/upstream interface(s); DHCPv6-PD route source prefixes from those interfaces validate LAN host IPv6 addresses.' \
		'DUID' \
		'IAID' \
		'Log Output' \
		'Name shown in tables, probes, and rule selectors.' \
		'Name shown in tables and rule selectors.' \
		'Only rules are editable on this page. Providers and sources live on the settings page.' \
			'Rule references use the latest saved providers and sources loaded with this page. Save and reload after adding referenced providers or sources on the settings page.'; do
			grep -nF "msgid \"$msgid\"" "$PO_FILE"
		done
		for msgid in 'OK' 'Success' 'Synced' 'Updated' 'Unchanged' 'Error' 'Failed' 'Invalid' 'Pending' 'Testing' 'Queued' 'Warning'; do
			grep -nF "msgid \"$msgid\"" "$PO_FILE"
		done
	! grep -nF 'msgid "Command"' "$PO_FILE"
	! grep -nF 'msgid "Shell command"' "$PO_FILE"
	! grep -nE "can be edited later|renaming referenced sections|changing visible names|rule sections|source sections|provider sections|referenced sections|provider section|internal numeric|内部数字|可见名称稍后可以编辑|重命名被引用|section" "$PO_FILE"
}
check_logs_boundary() {
	grep -nF 'qddns.getLogs' "$VIEW_DIR/logs.js"
	! grep -nE 'qddns\.(runRule|testRule|getRuleStatus)|form\.(Map|NamedSection|GridSection)' "$VIEW_DIR/logs.js"
}

check_theme_private_dependencies() {
	! grep -RniE 'argon|aurora|theme-argon|theme-aurora|\.argon-|\.aurora-|/luci-static/argon|/luci-static/aurora' "$VIEW_DIR"
}

check_acl_no_direct_log_file() {
	! grep -nE '/var/log/qddns|/tmp/.*qddns|\.log"[[:space:]]*:' "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/acl.d/luci-app-qddns.json"
}

check_theme_style() {
	! grep -nE '#111|#666|#e9eef5|linear-gradient\(' "$VIEW_DIR/overview.js"
}

check_common_style_tokens() {
	grep -nF "'--qddns-radius-md:0.5rem;'" "$VIEW_DIR/shared.js"
	grep -nF "letter-spacing:0;opacity:0.72;text-transform:none" "$VIEW_DIR/overview.js"
	! grep -nF "letter-spacing:0.04em" "$VIEW_DIR"/*.js
	! grep -nF "'--qddns-radius-sm:0.5rem;'" "$VIEW_DIR"/*.js
	! grep -nF "'--qddns-radius-md:0.75rem;'" "$VIEW_DIR"/*.js
}

SELFTEST_STATE_DIR=/tmp/qddns-selftest-state
SELFTEST_LOG_DIR=/tmp/qddns-selftest-log
SELFTEST_DRAFT_CONFIG=/tmp/qddns-selftest-draft-source.conf
SELFTEST_HTTP_PORT=35353
rm -rf "$SELFTEST_STATE_DIR" "$SELFTEST_LOG_DIR" "$SELFTEST_DRAFT_CONFIG"
mkdir -p "$SELFTEST_STATE_DIR" "$SELFTEST_LOG_DIR"

python3 - "$SELFTEST_STATE_DIR" "$SELFTEST_HTTP_PORT" <<'PYEOF' &
import http.server
import pathlib
import socketserver
import sys

state_dir = pathlib.Path(sys.argv[1])
port = int(sys.argv[2])

class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        return

    def do_GET(self):
        self.handle_request()

    def do_HEAD(self):
        self.handle_request(head=True)

    def do_POST(self):
        self.handle_request()

    def do_PUT(self):
        self.handle_request()

    def handle_request(self, head=False):
        if self.path.startswith('/lookup'):
            body = b'198.51.100.88\n'
            self.send_response(200)
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            if not head:
                self.wfile.write(body)
            return

        if self.path.startswith('/update'):
            length = int(self.headers.get('Content-Length') or '0')
            data = self.rfile.read(length) if length else b''
            (state_dir / 'update.txt').write_bytes(data)
            body = b'{"result":"updated"}'
            self.send_response(200)
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            if not head:
                self.wfile.write(body)
            return

        self.send_response(404)
        self.send_header('Content-Length', '0')
        self.end_headers()

class ReuseServer(socketserver.TCPServer):
    allow_reuse_address = True

with ReuseServer(('127.0.0.1', port), Handler) as httpd:
    httpd.serve_forever()
PYEOF
SELFTEST_HTTP_PID=$!
cleanup() {
	kill "$SELFTEST_HTTP_PID" 2>/dev/null || true
	wait "$SELFTEST_HTTP_PID" 2>/dev/null || true
	rm -f "$SELFTEST_DRAFT_CONFIG"
}
trap cleanup EXIT INT TERM
sleep 1

run_step 'Rust tests' cargo test -p qddns -- --nocapture
run_step 'Shell init syntax' sh -n "$ROOT_DIR/net/qddns/files/qddns.init"
run_step 'Package metadata guard' check_package_metadata
run_step 'LuCI required view files guard' check_required_view_files
run_step 'LuCI view syntax' check_view_syntax
run_step 'LuCI menu parent guard' grep -nF 'admin/services/qddns' "$MENU_FILE"
run_step 'LuCI menu firstchild guard' grep -nE '"type"[[:space:]]*:[[:space:]]*"firstchild"' "$MENU_FILE"
run_step 'LuCI menu preferred overview guard' grep -nE '"preferred"[[:space:]]*:[[:space:]]*"overview"' "$MENU_FILE"
run_step 'LuCI menu child pages guard' check_menu_child_pages
run_step 'LuCI zh_Hans PO exists guard' test -f "$PO_FILE"
run_step 'LuCI zh_Hans PO format guard' check_po_format
run_step 'LuCI zh_Hans core msgid guard' check_po_core_msgids
run_step 'LuCI zh_Hans core msgstr guard' grep -nE 'msgstr "概览"|msgstr "规则"|msgstr "设置"|msgstr "日志"|msgstr "运行"|msgstr "测试"|msgstr "运行摘要"|msgstr "来源探测"|msgstr "版本"' "$PO_FILE"
run_step 'LuCI zh_Hans critical msgstr guard' check_po_critical_zh_msgstrs
run_step 'LuCI view i18n hook guard' check_view_i18n_hooks
run_step 'LuCI overview primary cards guard' check_overview_primary_cards
run_step 'LuCI no duplicate internal page nav guard' check_no_internal_page_nav
run_step 'LuCI overview boundary guard' check_overview_boundary
run_step 'LuCI rules boundary guard' check_rules_boundary
run_step 'LuCI rules status badge tone guard' check_rules_status_badge_tones
run_step 'LuCI status display labels guard' check_status_display_labels
run_step 'LuCI rule wizard guard' check_rule_wizard
run_step 'LuCI rules compact table guard' check_rules_table_compactness
run_step 'LuCI settings boundary guard' check_settings_boundary
run_step 'LuCI settings source probe previewability guard' check_settings_source_probe_previewability
run_step 'Source probe no luci-rpc recursion guard' check_source_probe_no_luci_rpc_recursion
run_step 'LuCI name-visible numeric-hidden UI guard' check_name_visible_numeric_hidden_ui
run_step 'LuCI DHCPv6 lease fill UI guard' check_dhcpv6_lease_fill_ui
run_step 'LuCI DHCPv6 lease fill backend guard' check_dhcpv6_lease_fill_backend
run_step 'IPv6 WAN source prefix guard' check_ipv6_prefix_source_guard
run_step 'Default numeric section guard' check_default_config_numeric_sections
run_step 'Default DHCPv6 interface guard' check_default_config_dhcpv6_interface
run_step 'LuCI name-visible numeric-hidden PO guard' check_name_visible_numeric_hidden_po
run_step 'LuCI logs boundary guard' check_logs_boundary
run_step 'LuCI theme private dependency guard' check_theme_private_dependencies
run_step 'ucode export guard' grep -n 'return { qddns: methods };' "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
run_step 'ucode list_sources result guard' grep -n 'result: sources' "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
run_step 'LuCI list_sources shared RPC guard' grep -nF "const callSources = rpc.declare({ object: 'qddns', method: 'list_sources', expect: { result: [] } });" "$VIEW_DIR/shared.js"
run_step 'LuCI list_sources shared normalize guard' grep -nF "const sourceList = Array.isArray(sources) ? sources : sources?.result;" "$VIEW_DIR/shared.js"
run_step 'LuCI list_sources array normalize guard' grep -nF "sources: normalizeList(sourceList)" "$VIEW_DIR/shared.js"
run_step 'LuCI list_sources settings consumer guard' grep -nF "return qddns.normalizeCatalogState(data[0], data[1]);" "$VIEW_DIR/settings.js"
run_step 'LuCI list_sources rules consumer guard' grep -nF "const catalog = qddns.normalizeCatalogState(data[0], data[1]);" "$VIEW_DIR/rules.js"
run_step 'ucode secret guard' sh -c "! grep -nE 'api_token: section\.api_token|secret_id: section\.secret_id|secret_key: section\.secret_key|access_key_id: section\.access_key_id|access_key_secret: section\.access_key_secret|headers_json: section\.headers_json|body_template: section\.body_template' '$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc'"
run_step 'ucode fixed config bridge guard' grep -n "return exec_json_with_config('/etc/config/qddns', command);" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
run_step 'ucode no shell quote guard' sh -c "! grep -n 'function shell_quote' '$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc'"
run_step 'ucode probe type guard' grep -n 'is_probe_allowed_source_type' "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
run_step 'ucode probe command/script/public probe deny guard' sh -c "! grep -nE \"source_type == 'command'|source_type == 'script'|source_type == 'public_probe'\" '$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc'"
run_step 'ucode log bridge guard' grep -n 'exec_json(`logs' "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
run_step 'ucode no log path read guard' sh -c "! grep -nE 'log_dir|readlink\(|mkdir\(' '$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc'"
run_step 'acl mktemp exec guard' grep -n '"/bin/mktemp": \[ "exec" \]' "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/acl.d/luci-app-qddns.json"
run_step 'acl qddnsctl exec guard' grep -n '"/usr/bin/qddnsctl": \[ "exec" \]' "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/acl.d/luci-app-qddns.json"
run_step 'acl no direct log file guard' check_acl_no_direct_log_file
run_step 'acl boundary script guard' python3 "$ROOT_DIR/tests/check_acl_boundaries.py"
run_step 'rpcd redaction script guard' python3 "$ROOT_DIR/tests/check_rpcd_redaction.py"
run_step 'theme style guard' check_theme_style
run_step 'common style tokens guard' check_common_style_tokens
run_step 'Selftest source-only draft probe' check_source_only_draft_probe
run_step 'Selftest validate' cargo run --quiet --bin qddnsctl -- --config "$ROOT_DIR/tests/selftest.conf" validate
run_step 'Selftest sources list' sh -c "cargo run --quiet --bin qddnsctl -- --config '$ROOT_DIR/tests/selftest.conf' sources list | grep -qx 'wan4	local_addr'"
run_step 'Selftest source probe' sh -c "cargo run --quiet --bin qddnsctl -- --config '$ROOT_DIR/tests/selftest.conf' sources probe wan4 | grep -q '\"address\":\"198.51.100.77\"'"
run_step 'Selftest rules list' sh -c "cargo run --quiet --bin qddnsctl -- --config '$ROOT_DIR/tests/selftest.conf' rules list | grep -qx 'home	home	A	wan4'"
run_step 'Selftest rules test' sh -c "cargo run --quiet --bin qddnsctl -- --config '$ROOT_DIR/tests/selftest.conf' rules test home | grep -q '\"status\":\"success\"'"
run_step 'Selftest run rule' cargo run --quiet --bin qddnsctl -- --config "$ROOT_DIR/tests/selftest.conf" rules run home
run_step 'Selftest status' cargo run --quiet --bin qddnsctl -- --config "$ROOT_DIR/tests/selftest.conf" status
run_step 'Selftest daemon flag' sh -c "cargo run --quiet --bin qddnsctl -- --config '$ROOT_DIR/tests/selftest.conf' status | grep -q '\"running\":false'"
run_step 'Selftest version field' sh -c "cargo run --quiet --bin qddnsctl -- --config '$ROOT_DIR/tests/selftest.conf' status | grep -q '\"version\":\"0.2.0\"'"
run_step 'Selftest recent result status' sh -c "cargo run --quiet --bin qddnsctl -- --config '$ROOT_DIR/tests/selftest.conf' status | grep -q '\"status\":\"success\"'"
run_step 'Selftest rule status' cargo run --quiet --bin qddnsctl -- --config "$ROOT_DIR/tests/selftest.conf" rules status home
run_step 'Selftest rule status daemon flag' sh -c "cargo run --quiet --bin qddnsctl -- --config '$ROOT_DIR/tests/selftest.conf' rules status home | grep -q '\"running\":false'"
run_step 'Selftest rule status success' sh -c "cargo run --quiet --bin qddnsctl -- --config '$ROOT_DIR/tests/selftest.conf' rules status home | grep -q '\"status\":\"success\"'"
run_step 'Selftest logs' sh -c "cargo run --quiet --bin qddnsctl -- --config '$ROOT_DIR/tests/selftest.conf' logs home | grep -q '\"entries\":\['"
run_step 'Selftest invalid log scope' sh -c "! cargo run --quiet --bin qddnsctl -- --config '$ROOT_DIR/tests/selftest.conf' logs ../system >/tmp/qddns-selftest-invalid-log.out 2>/tmp/qddns-selftest-invalid-log.err"
run_step 'Selftest invalid log scope stderr' grep -n 'invalid log scope' /tmp/qddns-selftest-invalid-log.err
run_step 'Selftest artifacts' test -f "$SELFTEST_STATE_DIR/runtime.state"
run_step 'Selftest runtime artifact contract' sh -c "grep -q '\"daemon_running\":false' '$SELFTEST_STATE_DIR/runtime.state' && grep -q '\"status\":\"success\"' '$SELFTEST_STATE_DIR/runtime.state'"
run_step 'Selftest update artifact' sh -c "grep -q '\"ip\":\"198.51.100.77\"' '$SELFTEST_STATE_DIR/update.txt' && grep -q '\"record\":\"home\"' '$SELFTEST_STATE_DIR/update.txt' && grep -q '\"zone\":\"example.com\"' '$SELFTEST_STATE_DIR/update.txt'"
run_step 'Selftest log artifact' sh -c "test -f '$SELFTEST_LOG_DIR/home.log' && grep -q 'updated current=198.51.100.77 remote=198.51.100.77 detail=custom_http updated status=200' '$SELFTEST_LOG_DIR/home.log'"
