'use strict';
'require baseclass';
'require rpc';
'require ui';

const callOverview = rpc.declare({ object: 'qddns', method: 'get_overview', expect: {} });
const callRules = rpc.declare({ object: 'qddns', method: 'list_rules', expect: {} });
const callSources = rpc.declare({ object: 'qddns', method: 'list_sources', expect: { result: [] } });
const callDhcpv6Leases = rpc.declare({ object: 'qddns', method: 'list_dhcpv6_leases', params: ['mode'], expect: {} });
const callProbeSource = rpc.declare({ object: 'qddns', method: 'probe_source', params: ['id'], expect: {} });
const callRunRule = rpc.declare({ object: 'qddns', method: 'run_rule', params: ['id'], expect: {} });
const callTestRule = rpc.declare({ object: 'qddns', method: 'test_rule', params: ['id'], expect: {} });
const callGetLogs = rpc.declare({ object: 'qddns', method: 'get_logs', params: ['scope'], expect: {} });
const callGetRuleStatus = rpc.declare({ object: 'qddns', method: 'get_rule_status', params: ['id'], expect: {} });
const QDDNS_COMMON_STYLE_ID = 'qddns-common-style';
const QDDNS_COMMON_STYLE = [
	':root{',
		'--qddns-space-2:0.5rem;',
		'--qddns-space-3:0.75rem;',
		'--qddns-table-min:56rem;',
		'--qddns-form-table-min:72rem;',
		'--qddns-cell-min:8rem;',
		'--qddns-cell-wide:14rem;',
	'}',
	'.qddns-table-wrap{width:100%;max-width:100%;overflow-x:auto;-webkit-overflow-scrolling:touch}',
	'.qddns-table-wrap .qddns-table{width:100%;min-width:var(--qddns-table-min);margin-bottom:0;table-layout:auto}',
	'.qddns-table-wrap .qddns-table th,.qddns-table-wrap .qddns-table td{min-width:var(--qddns-cell-min);vertical-align:top;white-space:normal;word-break:normal;overflow-wrap:break-word}',
	'.qddns-table-wrap .qddns-table th{white-space:nowrap}',
	'.qddns-table-wrap .qddns-table th:first-child,.qddns-table-wrap .qddns-table td:first-child{white-space:nowrap}',
	'.qddns-table-wrap .qddns-table th:last-child,.qddns-table-wrap .qddns-table td:last-child{min-width:var(--qddns-cell-wide)}',
	'.qddns-wide-form{width:100%;max-width:100%;overflow-x:auto;-webkit-overflow-scrolling:touch}',
	'.qddns-wide-form .cbi-section-table{min-width:var(--qddns-form-table-min);table-layout:auto}',
	'.qddns-wide-form .cbi-section-table th,.qddns-wide-form .cbi-section-table td{min-width:var(--qddns-cell-min);vertical-align:top;white-space:normal;word-break:normal;overflow-wrap:break-word}',
	'.qddns-wide-form .cbi-section-table th{white-space:nowrap}',
	'.qddns-wide-form .cbi-section-table td:first-child,.qddns-wide-form .cbi-section-table td:last-child{white-space:nowrap}',
	'.qddns-wide-form .cbi-section-table .cbi-input-text,.qddns-wide-form .cbi-section-table .cbi-input-password,.qddns-wide-form .cbi-section-table .cbi-input-select{min-width:var(--qddns-cell-min);max-width:var(--qddns-cell-wide)}',
	'.qddns-wide-form .cbi-section-table input[type="checkbox"]{min-width:auto}',
	'@media (max-width: 768px){',
		':root{--qddns-table-min:48rem;--qddns-form-table-min:64rem}',
	'}'
].join('');

function normalizeList(items) {
	return Array.isArray(items) ? items : [];
}

function isElement(node, tagName) {
	return node && node.nodeType === 1 && node.tagName && node.tagName.toLowerCase() === tagName;
}

return baseclass.extend({
	overview: callOverview,
	listRules: callRules,
	listSources: callSources,
	listDhcpv6Leases: callDhcpv6Leases,
	probeSource: callProbeSource,
	runRule: callRunRule,
	testRule: callTestRule,
	getLogs: callGetLogs,
	getRuleStatus: callGetRuleStatus,

	normalizeList: normalizeList,

	ensureCommonStyle: function() {
		if (document.getElementById(QDDNS_COMMON_STYLE_ID))
			return;

		document.head.appendChild(E('style', { id: QDDNS_COMMON_STYLE_ID }, [QDDNS_COMMON_STYLE]));
	},

	normalizeRulesData: function(data) {
		return {
			providers: normalizeList(data?.providers),
			rules: normalizeList(data?.rules)
		};
	},

	normalizeCatalogState: function(rules, sources) {
		const sourceList = Array.isArray(sources) ? sources : sources?.result;

		return {
			rules: this.normalizeRulesData(rules),
			sources: normalizeList(sourceList)
		};
	},

	formatEpoch: function(epoch) {
		if (!epoch)
			return _('N/A');

		const date = new Date(epoch * 1000);
		return isNaN(date.getTime()) ? String(epoch) : date.toLocaleString();
	},

	sortNamedItems: function(items) {
		return normalizeList(items).slice().sort(function(left, right) {
			const leftLabel = left?.name || '';
			const rightLabel = right?.name || '';
			return leftLabel.localeCompare(rightLabel);
		});
	},

	statusTone: function(status) {
		const value = String(status || '').toLowerCase();

		if (['running', 'enabled', 'ok', 'success', 'synced', 'updated'].indexOf(value) > -1)
			return 'positive';

		if (['stopped', 'disabled', 'error', 'failed', 'invalid'].indexOf(value) > -1)
			return 'negative';

		if (['unknown', 'pending', 'testing', 'queued', 'warning'].indexOf(value) > -1)
			return 'warning';

		return 'neutral';
	},

	renderBadge: function(label, tone) {
		return E('span', { class: 'qddns-badge qddns-badge-' + (tone || 'neutral') }, label || '-');
	},

	renderStatusBadge: function(status, fallback) {
		const label = status || fallback || '-';
		return this.renderBadge(label, this.statusTone(label));
	},

	extractResultMessage: function(result, fallback) {
		if (typeof result == 'string' && result)
			return result;

		return result?.error || result?.detail || result?.message || fallback || _('Request failed');
	},

	isFailedResult: function(result) {
		return !result || result.ok === false;
	},

	withBusyButton: function(button, handler) {
		button.disabled = true;
		button.classList.add('qddns-busy');

		return Promise.resolve(handler()).finally(function() {
			button.disabled = false;
			button.classList.remove('qddns-busy');
		});
	},

	renderModalClose: function() {
		return E('div', { class: 'right' }, [
			E('button', { class: 'btn cbi-button', click: ui.hideModal }, [_('Close')])
		]);
	},

	showFailureModal: function(title, result, fallback) {
		ui.showModal(title, [
			E('div', { class: 'qddns-feedback qddns-feedback-negative' }, [
				E('strong', {}, _('Request failed')),
				E('p', {}, this.extractResultMessage(result, fallback))
			]),
			this.renderModalClose()
		]);
	},

	showInfoModal: function(title, nodes) {
		ui.showModal(title, nodes.concat([this.renderModalClose()]));
	},

	handleReadAction: function(button, title, handler, onSuccess, fallback) {
		return this.withBusyButton(button, L.bind(function() {
			return Promise.resolve(handler()).then(L.bind(function(result) {
				if (this.isFailedResult(result)) {
					this.showFailureModal(title, result, fallback);
					return result;
				}

				onSuccess(result);
				return result;
			}, this)).catch(L.bind(function(err) {
				this.showFailureModal(title, { error: this.extractResultMessage(err, fallback) }, fallback);
			}, this));
		}, this));
	},

	handleMutationAction: function(button, title, handler, onSuccess, fallback, refresh) {
		const refreshHandler = (typeof refresh == 'function') ? refresh : function() { return Promise.resolve(); };

		return this.withBusyButton(button, L.bind(function() {
			return Promise.resolve(handler()).then(L.bind(function(result) {
				return Promise.resolve(refreshHandler()).then(L.bind(function() {
					if (this.isFailedResult(result)) {
						this.showFailureModal(title, result, fallback);
						return result;
					}

					onSuccess(result);
					return result;
				}, this));
			}, this)).catch(L.bind(function(err) {
				return Promise.resolve(refreshHandler()).then(L.bind(function() {
					this.showFailureModal(title, { error: this.extractResultMessage(err, fallback) }, fallback);
				}, this));
			}, this));
		}, this));
	},

	renderTableSection: function(title, headers, rows, emptyText) {
		return E('div', { class: 'cbi-section qddns-panel' }, [
			E('h3', {}, title),
			this.renderTable(headers, rows, emptyText)
		]);
	},

	renderTable: function(headers, rows, emptyText) {
		this.ensureCommonStyle();

		const tableRows = normalizeList(rows);
		const tableChildren = [
			E('tr', { class: 'tr cbi-section-table-titles' }, headers.map(function(header) {
				return E('th', {}, header);
			}))
		];

		if (tableRows.length) {
			tableRows.forEach(function(row) {
				if (isElement(row, 'tr')) {
					tableChildren.push(row);
					return;
				}

				const cells = Array.isArray(row) ? row : [row];
				tableChildren.push(E('tr', {}, cells.map(function(cell) {
					return isElement(cell, 'td') ? cell : E('td', {}, Array.isArray(cell) ? cell : [cell]);
				})));
			});
		} else {
			tableChildren.push(E('tr', {}, [
				E('td', { colspan: headers.length, class: 'qddns-empty-cell' }, emptyText)
			]));
		}

		return E('div', { class: 'qddns-table-wrap' }, [
			E('table', { class: 'table cbi-section-table qddns-table' }, tableChildren)
		]);
	}
});
