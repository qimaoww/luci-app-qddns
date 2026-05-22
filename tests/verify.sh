#!/bin/sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
ROOT_DIR=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
VIEW_DIR="$ROOT_DIR/applications/luci-app-qddns/htdocs/luci-static/resources/view/qddns"
MENU_FILE="$ROOT_DIR/applications/luci-app-qddns/root/usr/share/luci/menu.d/luci-app-qddns.json"
PO_FILE="$ROOT_DIR/applications/luci-app-qddns/po/zh_Hans/qddns.po"
DEFAULT_CONFIG_FILE="$ROOT_DIR/net/qddns/files/qddns.config"

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
	for msgid in 'Overview' 'Rules' 'Settings' 'Logs' 'Run' 'Test' 'Probe' 'Close' 'Runtime Summary' 'Source Probe'; do
		grep -nF "msgid \"$msgid\"" "$PO_FILE"
	done
}

check_view_i18n_hooks() {
	grep -nF "_('Overview Console')" "$VIEW_DIR/overview.js"
	grep -nF "_('Runtime Summary')" "$VIEW_DIR/overview.js"
	grep -nF "_('Rule Console')" "$VIEW_DIR/rules.js"
	grep -nF "_('Run')" "$VIEW_DIR/rules.js"
	grep -nF "_('Test')" "$VIEW_DIR/rules.js"
	grep -nF "_('Settings')" "$VIEW_DIR/settings.js"
	grep -nF "_('Source Probe')" "$VIEW_DIR/settings.js"
	grep -nF "_('DHCPv6 DUID')" "$VIEW_DIR/settings.js"
	grep -nF "_('Logs')" "$VIEW_DIR/logs.js"
	grep -nF "_('Load Logs')" "$VIEW_DIR/logs.js"
	grep -nF "_('Close')" "$VIEW_DIR/shared.js"
	grep -nF "_('Request failed')" "$VIEW_DIR/shared.js"
	grep -nF "_('N/A')" "$VIEW_DIR/shared.js"
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
	grep -nF ".qddns-rules-form.qddns-wide-form{width:100%;max-width:100%;overflow-x:visible}" "$VIEW_DIR/rules.js"
	grep -nF ".qddns-rules-form.qddns-wide-form .cbi-map{width:100%;min-width:0}" "$VIEW_DIR/rules.js"
	grep -nF ".qddns-rules-form.qddns-wide-form .cbi-section-table{width:100%;min-width:0;table-layout:fixed}" "$VIEW_DIR/rules.js"
	grep -nF -- "--qddns-rule-toggle-width:6.5rem" "$VIEW_DIR/rules.js"
	grep -nF -- "--qddns-rule-type-width:8rem" "$VIEW_DIR/rules.js"
	grep -nF -- "--qddns-rule-action-min:10rem" "$VIEW_DIR/rules.js"
	! grep -nF -- "--qddns-rule-form-min" "$VIEW_DIR/rules.js"
	! grep -nF -- "--qddns-rule-form-max" "$VIEW_DIR/rules.js"
	python3 - <<'PYEOF'
from pathlib import Path
rules = Path('/home/qimaoaa/qddns-feed/applications/luci-app-qddns/htdocs/luci-static/resources/view/qddns/rules.js').read_text().splitlines()
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
	grep -nE 'qddns\.(runRule|testRule|getRuleStatus)' "$VIEW_DIR/rules.js"
	grep -nF "form.GridSection, 'rule'" "$VIEW_DIR/rules.js"
	! grep -nE "form\.(NamedSection, 'main'|GridSection, 'source'|GridSection, 'provider')" "$VIEW_DIR/rules.js"
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
	grep -nF "_('1. Address')" "$VIEW_DIR/rules.js"
	grep -nF "_('2. DNS')" "$VIEW_DIR/rules.js"
	grep -nF "_('3. Confirm')" "$VIEW_DIR/rules.js"
	grep -nF "_('Confirm and create the rule')" "$VIEW_DIR/rules.js"
	grep -nF "_('Rule name is generated automatically from the record.')" "$VIEW_DIR/rules.js"
	grep -nF "wizardRuleName" "$VIEW_DIR/rules.js"
	grep -nF "_('Next')" "$VIEW_DIR/rules.js"
	grep -nF "_('Back')" "$VIEW_DIR/rules.js"
	grep -nF "_('Provider and source choices show names only; rule links are saved automatically.')" "$VIEW_DIR/rules.js"
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
	grep -nF "_('Record type must match the selected source address family.')" "$VIEW_DIR/rules.js"
	grep -nF "return uci.save().then(function()" "$VIEW_DIR/rules.js"
	grep -nF "window.location.reload()" "$VIEW_DIR/rules.js"
	grep -nF "select.appendChild(E('option', { value: choice.id }, [choice.name || emptyText]))" "$VIEW_DIR/rules.js"
	python3 - <<'PYEOF'
from pathlib import Path
rules = Path('/home/qimaoaa/qddns-feed/applications/luci-app-qddns/htdocs/luci-static/resources/view/qddns/rules.js').read_text()
modal_start = rules.index('showRuleWizardModal: function')
modal_end = rules.index('renderRuleWizard: function')
modal = rules[modal_start:modal_end]
for field in ["this.renderWizardField(_('Record type')", "this.renderWizardField(_('Provider')", "this.renderWizardField(_('Source')", "this.renderWizardField(_('Zone')", "this.renderWizardField(_('Record name')"]:
	if modal.count(field) != 1:
		raise SystemExit(f'{field} must appear exactly once as a field in the modal wizard')
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

check_settings_boundary() {
	grep -nF "form.NamedSection, 'main'" "$VIEW_DIR/settings.js"
	grep -nF "form.GridSection, 'source'" "$VIEW_DIR/settings.js"
	grep -nF "form.GridSection, 'provider'" "$VIEW_DIR/settings.js"
	grep -nF 'qddns.probeSource' "$VIEW_DIR/settings.js"
	! grep -nE 'qddns\.(runRule|testRule|getRuleStatus)|form.GridSection, .rule.' "$VIEW_DIR/settings.js"
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
	grep -nF "o = s.option(widgets.DeviceSelect, 'interface', _('Interface'))" "$VIEW_DIR/settings.js"
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
	grep -nF "this.ruleLabel(latest.id)" "$VIEW_DIR/overview.js"
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
	grep -nF "const callDhcpv6Leases = rpc.declare({ object: 'qddns', method: 'list_dhcpv6_leases', expect: {} });" "$VIEW_DIR/shared.js"
	grep -nF "listDhcpv6Leases: callDhcpv6Leases" "$VIEW_DIR/shared.js"
	grep -nF "handleDhcpv6LeaseLoad" "$VIEW_DIR/settings.js"
	grep -nF "fillDhcpv6Lease" "$VIEW_DIR/settings.js"
	grep -nF "setSourceOptionValue" "$VIEW_DIR/settings.js"
	grep -nF "renderDhcpv6LeaseStatus" "$VIEW_DIR/settings.js"
	grep -nF "qddns.listDhcpv6Leases()" "$VIEW_DIR/settings.js"
	grep -nF "s.option(form.DummyValue, '_dhcpv6_status', _('Status'))" "$VIEW_DIR/settings.js"
	grep -nF "_('Read current DUID')" "$VIEW_DIR/settings.js"
	grep -nF "_('Read current DHCPv6 lease candidates, then choose one to fill the DUID source fields.')" "$VIEW_DIR/settings.js"
	grep -nF "_('Fill from this lease')" "$VIEW_DIR/settings.js"
	grep -nF "_('No DHCPv6 leases found.')" "$VIEW_DIR/settings.js"
	grep -nF "_('Selected DHCPv6 lease values have been filled. Save the source to keep them.')" "$VIEW_DIR/settings.js"
	grep -nF "options.duid" "$VIEW_DIR/settings.js"
	grep -nF "options.iaid" "$VIEW_DIR/settings.js"
	grep -nF "options.leaseFile" "$VIEW_DIR/settings.js"
	grep -nF "options.hostnameHint" "$VIEW_DIR/settings.js"
	grep -nF "options.prefixFilter" "$VIEW_DIR/settings.js"
	grep -nF "const widget = option.getUIElement(sectionId)" "$VIEW_DIR/settings.js"
	grep -nF "widget.setValue(normalized)" "$VIEW_DIR/settings.js"
	grep -nF "widget.node.setAttribute('data-changed', 'true')" "$VIEW_DIR/settings.js"
	grep -nF "widget.node.dispatchEvent(new CustomEvent('widget-change', { bubbles: true }))" "$VIEW_DIR/settings.js"
	grep -nF "getDhcpv6OptionSet" "$VIEW_DIR/settings.js"
	grep -nF "qddns-dhcpv6-lease-card" "$VIEW_DIR/settings.js"
	grep -nF "this.setSourceOptionValue(options.duid, sectionId, lease?.duid || '')" "$VIEW_DIR/settings.js"
	grep -nF "input.dispatchEvent(new Event('input', { bubbles: true }))" "$VIEW_DIR/settings.js"
	grep -nF "input.dispatchEvent(new Event('change', { bubbles: true }))" "$VIEW_DIR/settings.js"
	! grep -nF "qddns.renderTableSection(_('DHCPv6 leases')" "$VIEW_DIR/settings.js"
	! grep -nF "s.option(form.Button, '_dhcpv6_leases'" "$VIEW_DIR/settings.js"
	! grep -nF "querySelector('[id=\"%s\"]'" "$VIEW_DIR/settings.js"
	! grep -nF "read_direct('/tmp/odhcpd.leases" "$VIEW_DIR/settings.js" "$VIEW_DIR/shared.js"
}

check_dhcpv6_lease_fill_backend() {
	grep -nF "import { popen, readfile } from 'fs';" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "const dhcpv6_lease_file = '/tmp/odhcpd.leases';" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "const dhcpv6_lease_max_bytes = 262144;" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "const dhcpv6_lease_max_entries = 64;" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "const dhcpv6_lease_max_prefixes = 8;" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "function list_dhcpv6_leases()" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "DHCPv6 lease file is not available" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "content = substr(content, 0, dhcpv6_lease_max_bytes)" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "length(leases) >= dhcpv6_lease_max_entries" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "length(prefixes) >= dhcpv6_lease_max_prefixes" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "readfile(dhcpv6_lease_file)" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "list_dhcpv6_leases: {" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF "return list_dhcpv6_leases();" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	grep -nF '"list_dhcpv6_leases"' "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/acl.d/luci-app-qddns.json"
	grep -nF '"/tmp/odhcpd.leases": [ "read" ]' "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/acl.d/luci-app-qddns.json"
	! grep -nF "req.args.lease" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
	! grep -nF "req.args.path" "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
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
		'1. Address' \
		'2. DNS' \
		'3. Confirm' \
		'Choose the address to publish' \
		'Choose where to update DNS' \
		'Confirm and create the rule' \
		'Rule name is generated automatically from the record.' \
		'Back' \
		'Next' \
		'Add DDNS rule' \
		'No providers available' \
		'No sources available' \
		'Provider and source choices show names only; rule links are saved automatically.' \
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
		'Command' \
		'Status' \
		'Read current DUID' \
		'Read current DHCPv6 lease candidates, then choose one to fill the DUID source fields.' \
		'Choose a current DUID to fill DUID, IAID, hostname hint, and prefix filter.' \
		'Fill from this lease' \
		'No DHCPv6 leases found.' \
		'Selected DHCPv6 lease values have been filled. Save the source to keep them.' \
		'DHCPv6 leases' \
		'Unable to load DHCPv6 leases.' \
		'Unnamed host' \
		'Hostname' \
		'Prefix' \
		'DUID' \
		'IAID' \
		'Log Output' \
		'Name shown in tables, probes, and rule selectors.' \
		'Name shown in tables and rule selectors.' \
		'Only rules are editable on this page. Providers and sources live on the settings page.' \
		'Rule references use the latest saved providers and sources loaded with this page. Save and reload after adding referenced providers or sources on the settings page.'; do
		grep -nF "msgid \"$msgid\"" "$PO_FILE"
	done
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

SELFTEST_STATE_DIR=/tmp/qddns-selftest-state
SELFTEST_LOG_DIR=/tmp/qddns-selftest-log
rm -rf "$SELFTEST_STATE_DIR" "$SELFTEST_LOG_DIR"
mkdir -p "$SELFTEST_STATE_DIR" "$SELFTEST_LOG_DIR"
printf '198.51.100.88\n' > "$SELFTEST_STATE_DIR/lookup.txt"

run_step 'Rust tests' cargo test -p qddns -- --nocapture
run_step 'Shell init syntax' sh -n "$ROOT_DIR/net/qddns/files/qddns.init"
run_step 'LuCI required view files guard' check_required_view_files
run_step 'LuCI view syntax' check_view_syntax
run_step 'LuCI menu parent guard' grep -nF 'admin/services/qddns' "$MENU_FILE"
run_step 'LuCI menu firstchild guard' grep -nE '"type"[[:space:]]*:[[:space:]]*"firstchild"' "$MENU_FILE"
run_step 'LuCI menu preferred overview guard' grep -nE '"preferred"[[:space:]]*:[[:space:]]*"overview"' "$MENU_FILE"
run_step 'LuCI menu child pages guard' check_menu_child_pages
run_step 'LuCI zh_Hans PO exists guard' test -f "$PO_FILE"
run_step 'LuCI zh_Hans PO format guard' check_po_format
run_step 'LuCI zh_Hans core msgid guard' check_po_core_msgids
run_step 'LuCI zh_Hans core msgstr guard' grep -nE 'msgstr "概览"|msgstr "规则"|msgstr "设置"|msgstr "日志"|msgstr "运行"|msgstr "测试"|msgstr "运行摘要"|msgstr "来源探测"' "$PO_FILE"
run_step 'LuCI view i18n hook guard' check_view_i18n_hooks
run_step 'LuCI no duplicate internal page nav guard' check_no_internal_page_nav
	run_step 'LuCI overview boundary guard' check_overview_boundary
	run_step 'LuCI rules boundary guard' check_rules_boundary
	run_step 'LuCI rule wizard guard' check_rule_wizard
	run_step 'LuCI rules compact table guard' check_rules_table_compactness
run_step 'LuCI settings boundary guard' check_settings_boundary
run_step 'LuCI name-visible numeric-hidden UI guard' check_name_visible_numeric_hidden_ui
run_step 'LuCI DHCPv6 lease fill UI guard' check_dhcpv6_lease_fill_ui
run_step 'LuCI DHCPv6 lease fill backend guard' check_dhcpv6_lease_fill_backend
run_step 'Default numeric section guard' check_default_config_numeric_sections
run_step 'LuCI name-visible numeric-hidden PO guard' check_name_visible_numeric_hidden_po
run_step 'LuCI logs boundary guard' check_logs_boundary
run_step 'LuCI theme private dependency guard' check_theme_private_dependencies
run_step 'ucode export guard' grep -n 'return { qddns: methods };' "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
run_step 'ucode list_sources result guard' grep -n 'result: sources' "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
run_step 'LuCI list_sources shared RPC guard' grep -nF "const callSources = rpc.declare({ object: 'qddns', method: 'list_sources', expect: { result: [] } });" "$VIEW_DIR/shared.js"
	run_step 'LuCI list_sources shared normalize guard' grep -nF "const sourceList = Array.isArray(sources) ? sources : sources?.result;" "$VIEW_DIR/shared.js"
	run_step 'LuCI list_sources array normalize guard' grep -nF "sources: normalizeList(sourceList)" "$VIEW_DIR/shared.js"
run_step 'LuCI list_sources settings consumer guard' grep -nF "return qddns.normalizeCatalogState(data[0], data[1]);" "$VIEW_DIR/settings.js"
run_step 'LuCI list_sources rules consumer guard' grep -nF "return qddns.normalizeCatalogState(data[0], data[1]);" "$VIEW_DIR/rules.js"
run_step 'ucode secret guard' sh -c "! grep -nE 'api_token: section\.api_token|secret_id: section\.secret_id|secret_key: section\.secret_key|access_key_id: section\.access_key_id|access_key_secret: section\.access_key_secret|headers_json: section\.headers_json|body_template: section\.body_template' '$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc'"
run_step 'ucode log bridge guard' grep -n 'exec_json(`--config /etc/config/qddns logs' "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc"
run_step 'ucode no log path read guard' sh -c "! grep -nE 'log_dir|readlink\(|stat\(|unlink\(|mkdir\(' '$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/ucode/qddns.uc'"
run_step 'acl qddnsctl exec guard' grep -n '"/usr/bin/qddnsctl": \[ "exec" \]' "$ROOT_DIR/applications/luci-app-qddns/root/usr/share/rpcd/acl.d/luci-app-qddns.json"
run_step 'acl no direct log file guard' check_acl_no_direct_log_file
run_step 'theme style guard' check_theme_style
run_step 'Selftest validate' cargo run --quiet --bin qddnsctl -- --config "$ROOT_DIR/tests/selftest.conf" validate
run_step 'Selftest sources list' sh -c "cargo run --quiet --bin qddnsctl -- --config '$ROOT_DIR/tests/selftest.conf' sources list | grep -qx 'wan4	local_addr'"
run_step 'Selftest source probe' sh -c "cargo run --quiet --bin qddnsctl -- --config '$ROOT_DIR/tests/selftest.conf' sources probe wan4 | grep -q '\"address\":\"198.51.100.77\"'"
run_step 'Selftest rules list' sh -c "cargo run --quiet --bin qddnsctl -- --config '$ROOT_DIR/tests/selftest.conf' rules list | grep -qx 'home	home	A	wan4'"
run_step 'Selftest rules test' sh -c "cargo run --quiet --bin qddnsctl -- --config '$ROOT_DIR/tests/selftest.conf' rules test home | grep -q '\"status\":\"success\"'"
run_step 'Selftest run rule' cargo run --quiet --bin qddnsctl -- --config "$ROOT_DIR/tests/selftest.conf" rules run home
run_step 'Selftest status' cargo run --quiet --bin qddnsctl -- --config "$ROOT_DIR/tests/selftest.conf" status
run_step 'Selftest daemon flag' sh -c "cargo run --quiet --bin qddnsctl -- --config '$ROOT_DIR/tests/selftest.conf' status | grep -q '\"running\":false'"
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
