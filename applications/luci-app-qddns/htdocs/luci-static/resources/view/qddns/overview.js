'use strict';
'require view';
'require poll';
'require view.qddns.shared as qddns';

const QDDNS_STYLE_ID = 'qddns-overview-style';
const QDDNS_STYLE = [
	'.qddns-dashboard{margin-bottom:var(--qddns-space-5)}',
	'.qddns-dashboard .qddns-panel,.qddns-dashboard .qddns-card{margin-bottom:var(--qddns-space-4)}',
	'.qddns-cards{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:var(--qddns-space-3)}',
	'.qddns-summary-grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(12rem,1fr));gap:var(--qddns-space-3)}',
	'.qddns-card,.qddns-summary-item{padding:var(--qddns-space-4);border:1px solid var(--qddns-border);border-radius:var(--qddns-radius-md);background:var(--qddns-surface)}',
	'.qddns-card{display:flex;flex-direction:column;gap:var(--qddns-space-2);min-height:7.5rem;justify-content:center}',
	'.qddns-card-label,.qddns-summary-item strong{font-size:0.75rem;letter-spacing:0;opacity:0.72;text-transform:none}',
	'.qddns-card-value{font-size:1.5rem;font-weight:600;line-height:1.3;word-break:break-word}',
	'.qddns-summary-item{display:flex;flex-direction:column;gap:var(--qddns-space-1)}',
	'.qddns-summary-item span{font-size:1rem;line-height:1.4}',
	'@media (max-width: 1100px){',
		'.qddns-cards{grid-template-columns:repeat(2,minmax(0,1fr))}',
	'}',
	'@media (max-width: 768px){',
		'.qddns-card,.qddns-summary-item{padding:var(--qddns-space-3)}',
		'.qddns-card-value{font-size:1.25rem}',
		'.qddns-cards{grid-template-columns:1fr}',
	'}'
].join('');

return view.extend({
	handleSaveApply: null,
	handleSave: null,
	handleReset: null,

	loadRuntimeState: function() {
		return L.resolveDefault(qddns.overview(), {});
	},

	load: function() {
		return Promise.all([
			this.loadRuntimeState(),
			L.resolveDefault(qddns.listRules(), { rules: [] })
		]).then(function(data) {
			return {
				overview: data[0] || {},
				rules: qddns.normalizeRulesData(data[1]).rules
			};
		});
	},

	buildRuleLabels: function(rules) {
		const labels = {};

		qddns.normalizeList(rules).forEach(function(rule) {
			if (rule?.id)
				labels[rule.id] = String(rule.name || '').trim() || _('Unnamed rule');
		});

		return labels;
	},

	ruleLabel: function(ruleId) {
		return this.ruleLabels?.[ruleId] || _('Unnamed rule');
	},

	ensureDashboardStyle: function() {
		qddns.ensureCommonStyle();

		if (document.getElementById(QDDNS_STYLE_ID))
			return;

		document.head.appendChild(E('style', { id: QDDNS_STYLE_ID }, [QDDNS_STYLE]));
	},

	replaceDashboard: function() {
		const root = document.getElementById('qddns-dashboard');
		if (root)
			root.replaceWith(this.renderDashboard(this.runtimeData));
	},

	refreshRuntime: function() {
		return this.loadRuntimeState().then(L.bind(function(overview) {
			this.runtimeData = overview || {};
			this.replaceDashboard();
			return this.runtimeData;
		}, this));
	},

	renderDashboardIntro: function() {
		return qddns.renderPageHeader({
			active: 'overview',
			title: _('Overview'),
			description: _('Runtime widgets refresh automatically. Use the dedicated rules, settings, and logs pages for actions, configuration, and diagnostics.')
		});
	},

	renderOverviewCards: function(overview) {
		const status = overview.status || {};
		const cards = [
			{ label: _('Daemon'), value: qddns.renderStatusBadge(status.running ? _('Running') : _('Stopped'), null, status.running ? 'running' : 'stopped') },
			{ label: _('Version'), value: status.version || '-' },
			{ label: _('Enabled Rules'), value: String(status.enabled_rules || 0) },
			{ label: _('Rules'), value: String(status.rules || 0) }
		];

		return E('div', { class: 'qddns-cards' }, cards.map(function(card) {
			return E('div', { class: 'cbi-section qddns-card' }, [
				E('div', { class: 'qddns-card-label' }, card.label),
				E('div', { class: 'qddns-card-value' }, Array.isArray(card.value) ? card.value : [card.value])
			]);
		}));
	},

	renderRecentResults: function(overview) {
		const results = overview.status?.recent_results || [];

		return qddns.renderTableSection(_('Recent Results'), [
			_('Rule'), _('Status'), _('Current IP'), _('Remote IP'), _('Result'), _('Last Check')
		], results.map(L.bind(function(item) {
				return [
					this.ruleLabel(item.id),
					qddns.renderStatusBadge(item.status, _('Unknown')),
					item.current_ip || '-',
					item.remote_ip || '-',
					qddns.resultLabel(item.last_result) || item.last_error || '-',
					item.last_check ? qddns.formatEpoch(item.last_check) : '-'
				];
			}, this)), _('No runtime results yet'));
	},

	renderStatusSummary: function(overview) {
		const status = overview.status || {};
		const enabled = overview.main?.enabled ? _('Enabled') : _('Disabled');

		return E('div', { class: 'cbi-section qddns-panel qddns-summary' }, [
			E('h3', {}, _('Runtime Summary')),
			E('div', { class: 'qddns-summary-grid' }, [
				E('div', { class: 'qddns-summary-item' }, [E('strong', {}, _('Service')), E('span', {}, [qddns.renderStatusBadge(status.running ? _('Running') : _('Stopped'), null, status.running ? 'running' : 'stopped')])]),
				E('div', { class: 'qddns-summary-item' }, [E('strong', {}, _('Config')), E('span', {}, [qddns.renderStatusBadge(enabled, null, overview.main?.enabled ? 'enabled' : 'disabled')])]),
				E('div', { class: 'qddns-summary-item' }, [E('strong', {}, _('Sources')), E('span', {}, String(status.sources || 0))]),
				E('div', { class: 'qddns-summary-item' }, [E('strong', {}, _('Providers')), E('span', {}, String(status.providers || 0))])
			])
		]);
	},

	renderDashboard: function(data) {
		this.ensureDashboardStyle();

		return E('div', { id: 'qddns-dashboard', class: 'qddns-dashboard' }, [
			this.renderDashboardIntro(),
			this.renderOverviewCards(data || {}),
			this.renderStatusSummary(data || {}),
			this.renderRecentResults(data || {})
		]);
	},

	render: function(data) {
		this.runtimeData = data?.overview || {};
		this.ruleLabels = this.buildRuleLabels(data?.rules);
		this.ensureDashboardStyle();

		poll.add(L.bind(function() {
			return this.refreshRuntime();
		}, this));

		return this.renderDashboard(this.runtimeData);
	}
});
