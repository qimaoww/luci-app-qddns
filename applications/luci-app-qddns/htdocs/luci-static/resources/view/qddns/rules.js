'use strict';
'require view';
'require uci';
'require ui';
'require form';
'require view.qddns.shared as qddns';

const QDDNS_STYLE_ID = 'qddns-rules-style';
const QDDNS_STYLE = [
	':root{',
		'--qddns-space-1:0.25rem;',
		'--qddns-space-2:0.5rem;',
		'--qddns-space-3:0.75rem;',
		'--qddns-space-4:1rem;',
		'--qddns-radius-sm:0.5rem;',
		'--qddns-radius-md:0.75rem;',
		'--qddns-border:rgba(127,127,127,0.24);',
		'--qddns-surface:rgba(127,127,127,0.08);',
		'--qddns-surface-strong:rgba(127,127,127,0.14);',
		'--qddns-positive:rgba(46,159,98,0.18);',
		'--qddns-positive-text:rgb(35,115,72);',
		'--qddns-negative:rgba(200,73,73,0.16);',
		'--qddns-negative-text:rgb(146,47,47);',
		'--qddns-warning:rgba(220,150,35,0.18);',
		'--qddns-warning-text:rgb(145,97,14);',
		'--qddns-neutral:rgba(127,127,127,0.12);',
		'--qddns-neutral-text:inherit;',
		'--qddns-rule-console-min:46rem;',
		'--qddns-rule-toggle-width:6.5rem;',
		'--qddns-rule-type-width:8rem;',
		'--qddns-rule-action-min:10rem;',
	'}',
	'.qddns-rules-page{margin-bottom:var(--qddns-space-4)}',
	'.qddns-panel{margin-bottom:var(--qddns-space-4);padding:var(--qddns-space-4);border:1px solid var(--qddns-border);border-radius:var(--qddns-radius-md);background:var(--qddns-surface)}',
	'.qddns-table-wrap{overflow-x:auto}',
	'.qddns-table-wrap .table{margin-bottom:0}',
	'.qddns-rules-console-table .qddns-table{min-width:var(--qddns-rule-console-min);table-layout:fixed}',
	'.qddns-rules-console-table .qddns-table th,.qddns-rules-console-table .qddns-table td{min-width:0;overflow-wrap:anywhere}',
	'.qddns-rules-console-table .qddns-table th:last-child,.qddns-rules-console-table .qddns-table td:last-child{width:var(--qddns-rule-action-min);min-width:var(--qddns-rule-action-min)}',
	'.qddns-rules-form.qddns-wide-form{width:100%;max-width:100%;overflow-x:visible}',
	'.qddns-rules-form.qddns-wide-form .cbi-map{width:100%;min-width:0}',
	'.qddns-rules-form.qddns-wide-form .cbi-section-table{width:100%;min-width:0;table-layout:fixed}',
	'.qddns-rules-form.qddns-wide-form .cbi-section-table th,.qddns-rules-form.qddns-wide-form .cbi-section-table td{min-width:0;vertical-align:middle;white-space:normal;overflow-wrap:anywhere;word-break:break-word}',
	'.qddns-rules-form.qddns-wide-form .cbi-section-table th{white-space:nowrap}',
	'.qddns-rules-form.qddns-wide-form .cbi-section-table th:first-child,.qddns-rules-form.qddns-wide-form .cbi-section-table td:first-child{width:auto;white-space:normal;font-weight:600}',
	'.qddns-rules-form.qddns-wide-form .cbi-section-table th:nth-child(2),.qddns-rules-form.qddns-wide-form .cbi-section-table td:nth-child(2){width:var(--qddns-rule-toggle-width);white-space:nowrap;text-align:center}',
	'.qddns-rules-form.qddns-wide-form .cbi-section-table th:nth-child(3),.qddns-rules-form.qddns-wide-form .cbi-section-table td:nth-child(3){width:var(--qddns-rule-type-width);white-space:nowrap}',
	'.qddns-rules-form.qddns-wide-form .cbi-section-table td:last-child{width:var(--qddns-rule-action-min);min-width:var(--qddns-rule-action-min);white-space:nowrap;text-align:right}',
	'.qddns-rules-form.qddns-wide-form .cbi-section-table .cbi-button{margin:0 var(--qddns-space-1) var(--qddns-space-1) 0}',
	'.qddns-rules-form.qddns-wide-form .cbi-section-table .cbi-input-text,.qddns-rules-form.qddns-wide-form .cbi-section-table .cbi-input-select{width:100%;min-width:0;max-width:100%}',
	'.qddns-rules-form.qddns-wide-form .cbi-section-table input[type="checkbox"]{min-width:auto}',
	'.qddns-actions{display:flex;flex-wrap:wrap;gap:var(--qddns-space-2);max-width:100%}',
	'.qddns-actions .cbi-button{margin:0;max-width:100%;white-space:normal}',
	'.qddns-actions .cbi-button.qddns-busy{opacity:0.7;cursor:progress}',
	'.qddns-rule-wizard-entry{display:flex;flex-wrap:wrap;align-items:center;justify-content:space-between;gap:var(--qddns-space-3)}',
	'.qddns-rule-wizard-entry-text{display:grid;gap:var(--qddns-space-1);min-width:16rem;max-width:42rem}',
	'.qddns-rule-wizard-entry-text h3,.qddns-rule-wizard-entry-text p{margin:0}',
	'.qddns-rule-wizard-primary{font-size:1rem;font-weight:700;padding:var(--qddns-space-3) var(--qddns-space-4)}',
	'.qddns-rule-wizard-modal{display:grid;gap:var(--qddns-space-4);min-width:min(42rem,90vw)}',
	'.qddns-rule-wizard-steps{display:flex;flex-wrap:wrap;gap:var(--qddns-space-2)}',
	'.qddns-rule-wizard-step{padding:var(--qddns-space-1) var(--qddns-space-2);border:1px solid var(--qddns-border);border-radius:999px;background:var(--qddns-neutral)}',
	'.qddns-rule-wizard-step.is-active{font-weight:700;background:var(--qddns-surface-strong);border-color:currentColor}',
	'.qddns-rule-wizard-grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(15rem,1fr));gap:var(--qddns-space-3)}',
	'.qddns-rule-wizard-field{display:flex;flex-direction:column;gap:var(--qddns-space-1)}',
	'.qddns-rule-wizard-field label{font-weight:600}',
	'.qddns-rule-wizard-field .cbi-input-text,.qddns-rule-wizard-field .cbi-input-select{width:100%;max-width:100%}',
	'.qddns-rule-wizard-switch{display:flex;align-items:center;gap:var(--qddns-space-2);min-height:2.4rem}',
	'.qddns-rule-wizard-summary{display:grid;gap:var(--qddns-space-2);padding:var(--qddns-space-3);border:1px solid var(--qddns-border);border-radius:var(--qddns-radius-sm);background:var(--qddns-surface-strong)}',
	'.qddns-rule-wizard-summary p{margin:0}',
	'.qddns-rule-wizard-source-ip{display:inline-block;max-width:100%;overflow-wrap:anywhere}',
	'.qddns-rule-wizard-source-ip[data-tone="warning"]{opacity:0.78}',
	'.qddns-rule-wizard-source-ip[data-tone="negative"]{color:var(--qddns-negative-text)}',
	'.qddns-rule-wizard-feedback{margin-top:var(--qddns-space-3)}',
	'.qddns-rule-wizard-modal .qddns-actions{justify-content:flex-end}',
	'.qddns-empty-cell{text-align:center;opacity:0.72;padding:var(--qddns-space-4)}',
	'.qddns-log-output{margin:0;max-height:20rem;overflow:auto;padding:var(--qddns-space-4);border:1px solid var(--qddns-border);border-radius:var(--qddns-radius-sm);background:var(--qddns-surface-strong);white-space:pre-wrap;word-break:break-word}',
	'.qddns-badge{display:inline-flex;align-items:center;justify-content:center;min-height:2rem;padding:0 var(--qddns-space-3);border-radius:999px;font-size:0.8125rem;font-weight:600;line-height:1.4;border:1px solid transparent}',
	'.qddns-badge-positive{background:var(--qddns-positive);border-color:var(--qddns-positive);color:var(--qddns-positive-text)}',
	'.qddns-badge-negative{background:var(--qddns-negative);border-color:var(--qddns-negative);color:var(--qddns-negative-text)}',
	'.qddns-badge-warning{background:var(--qddns-warning);border-color:var(--qddns-warning);color:var(--qddns-warning-text)}',
	'.qddns-badge-neutral{background:var(--qddns-neutral);border-color:var(--qddns-border);color:var(--qddns-neutral-text)}',
	'.qddns-feedback{display:flex;flex-direction:column;gap:var(--qddns-space-2);padding:var(--qddns-space-4);border:1px solid var(--qddns-border);border-radius:var(--qddns-radius-sm);background:var(--qddns-surface)}',
	'.qddns-feedback-negative{border-color:var(--qddns-negative);background:var(--qddns-negative)}',
	'.qddns-modal-meta{display:grid;gap:var(--qddns-space-2);margin-bottom:var(--qddns-space-4)}',
	'.qddns-modal-meta p{margin:0}',
	'@media (max-width: 768px){',
		'.qddns-panel{padding:var(--qddns-space-3)}',
		':root{--qddns-rule-console-min:40rem;--qddns-rule-action-min:8.5rem}',
	'}'
].join('');

return view.extend({
	loadRuntimeState: function() {
		return L.resolveDefault(qddns.overview(), {});
	},

	loadCatalogState: function() {
		return Promise.all([
			L.resolveDefault(qddns.listRules(), { providers: [], rules: [] }),
			L.resolveDefault(qddns.listSources(), { result: [] }),
			uci.load('qddns')
		]).then(function(data) {
			return qddns.normalizeCatalogState(data[0], data[1]);
		});
	},

	buildPageData: function(runtime, catalog) {
		return {
			runtime: runtime || {},
			catalog: {
				rules: qddns.normalizeRulesData(catalog?.rules),
				sources: qddns.normalizeList(catalog?.sources)
			}
		};
	},

	load: function() {
		return Promise.all([
			this.loadRuntimeState(),
			this.loadCatalogState()
		]).then(L.bind(function(data) {
			return this.buildPageData(data[0], data[1]);
		}, this));
	},

	ensurePageStyle: function() {
		if (document.getElementById(QDDNS_STYLE_ID))
			return;

		document.head.appendChild(E('style', { id: QDDNS_STYLE_ID }, [QDDNS_STYLE]));
	},

	getRuleStates: function() {
		return this.pageData?.runtime?.status?.rule_states || {};
	},


	nextNumericSectionId: function() {
		const used = {};
		const sections = uci.sections('qddns') || [];

		sections.forEach(function(section) {
			used[section['.name']] = true;
		});

		for (let index = 1; true; index++) {
			const candidate = String(index);
			if (!used[candidate])
				return candidate;
		}
	},

	getRuleLabel: function(rule) {
		return rule?.name || _('Unnamed rule');
	},

	findById: function(items, id) {
		const list = qddns.normalizeList(items);
		for (let index = 0; index < list.length; index++) {
			if (list[index]?.id === id)
				return list[index];
		}
		return null;
	},

	getSourceLabel: function(sourceId) {
		const source = this.findById(this.pageData?.catalog?.sources, sourceId);
		return source?.name || _('Unnamed source');
	},

	getProviderLabel: function(providerId) {
		const provider = this.findById(this.pageData?.catalog?.rules?.providers, providerId);
		return provider?.name || _('Unnamed provider');
	},

	renderWizardField: function(label, control, description) {
		const nodes = [
			E('label', {}, label),
			control
		];

		if (description)
			nodes.push(E('div', { class: 'cbi-value-description' }, description));

		return E('div', { class: 'qddns-rule-wizard-field' }, nodes);
	},

	renderWizardSelect: function(choices, emptyText) {
		const select = E('select', { class: 'cbi-input-select' });
		const list = qddns.normalizeList(choices);

		if (!list.length) {
			select.appendChild(E('option', { value: '' }, [emptyText]));
			select.disabled = true;
			return select;
		}

		list.forEach(function(choice) {
			select.appendChild(E('option', { value: choice.id }, [choice.name || emptyText]));
		});

		return select;
	},

	renderWizardSourceIp: function(statusNode) {
		return this.renderWizardField(_('Source IP'), statusNode);
	},

	wizardValue: function(control) {
		return String(control?.value || '').trim();
	},

	wizardSelectedText: function(control, fallback) {
		const option = control?.options?.[control.selectedIndex];
		return String(option?.textContent || fallback || '').trim();
	},

	wizardRuleName: function(recordName, zone, recordType) {
		return '%s.%s %s'.format(recordName, zone, recordType);
	},

	setWizardStep: function(modal, stepIndex) {
		const panels = modal.querySelectorAll('[data-wizard-panel]');
		const steps = modal.querySelectorAll('[data-wizard-step]');

		panels.forEach(function(panel, index) {
			panel.style.display = index === stepIndex ? '' : 'none';
		});

		steps.forEach(function(step, index) {
			step.classList.toggle('is-active', index === stepIndex);
		});
	},

	sourceFamily: function(sourceId) {
		const source = this.findById(this.pageData?.catalog?.sources, sourceId);
		return String(source?.family || '').toLowerCase();
	},

	isProbeableSourceType: function(sourceType) {
		return ['local_addr', 'interface', 'dhcpv6_duid', 'dhcpv6_mac'].indexOf(sourceType) > -1;
	},

	wizardSourceFamily: function(fields, sourceId) {
		return String(fields.source?.getAttribute('data-probed-family') || this.sourceFamily(sourceId)).toLowerCase();
	},

	setWizardFeedback: function(feedback, message) {
		feedback.textContent = message;
		feedback.classList.add('alert-message', 'warning');
	},

	resetWizardFeedback: function(feedback) {
		feedback.textContent = _('Choose the source IP first, then choose the DNS location.');
		feedback.classList.remove('alert-message', 'warning');
	},

	validateWizardStep: function(fields, feedback, stepIndex) {
		this.resetWizardFeedback(feedback);

		if (stepIndex === 0) {
			const source = this.wizardValue(fields.source);
			const recordType = this.wizardValue(fields.recordType) || 'A';

			if (!source) {
				this.setWizardFeedback(feedback, _('Source is required.'));
				return false;
			}

			if (fields.source?.getAttribute('data-source-ip-loading') === '1') {
				this.setWizardFeedback(feedback, _('Source IP is still loading.'));
				return false;
			}

			const family = this.wizardSourceFamily(fields, source);
			if ((recordType === 'A' && family === 'ipv6') || (recordType === 'AAAA' && family === 'ipv4')) {
				this.setWizardFeedback(feedback, _('Record type must match the selected source address family.'));
				return false;
			}
		}

		if (stepIndex === 1 && (!this.wizardValue(fields.provider) || !this.wizardValue(fields.zone) || !this.wizardValue(fields.recordName))) {
			this.setWizardFeedback(feedback, _('Provider, zone, and record name are required.'));
			return false;
		}

		return true;
	},

	createRuleFromWizard: function(fields, feedback, button) {
		const provider = this.wizardValue(fields.provider);
		const source = this.wizardValue(fields.source);
		const zone = this.wizardValue(fields.zone);
		const recordName = this.wizardValue(fields.recordName);
		const recordType = this.wizardValue(fields.recordType) || 'A';
		const ruleName = this.wizardRuleName(recordName, zone, recordType);

		if (!provider || !source || !zone || !recordName) {
			feedback.textContent = _('Provider, source, zone, and record name are required.');
			feedback.classList.add('alert-message', 'warning');
			return Promise.resolve();
		}

		const family = this.wizardSourceFamily(fields, source);
		if ((recordType === 'A' && family === 'ipv6') || (recordType === 'AAAA' && family === 'ipv4')) {
			feedback.textContent = _('Record type must match the selected source address family.');
			feedback.classList.add('alert-message', 'warning');
			return Promise.resolve();
		}

		button.disabled = true;
		button.classList.add('qddns-busy');
		feedback.textContent = _('Saving rule...');
		feedback.classList.remove('alert-message', 'warning');

		const sectionId = uci.add('qddns', 'rule', this.nextNumericSectionId());
		uci.set('qddns', sectionId, 'name', ruleName);
		uci.set('qddns', sectionId, 'enabled', fields.enabled.checked ? '1' : '0');
		uci.set('qddns', sectionId, 'record_type', recordType);
		uci.set('qddns', sectionId, 'provider', provider);
		uci.set('qddns', sectionId, 'source', source);
		uci.set('qddns', sectionId, 'zone', zone);
		uci.set('qddns', sectionId, 'record_name', recordName);
		uci.set('qddns', sectionId, 'proxied', '0');
		uci.set('qddns', sectionId, 'check_interval', '60');
		uci.set('qddns', sectionId, 'force_interval', '3600');
		uci.set('qddns', sectionId, 'retry_count', '3');
		uci.set('qddns', sectionId, 'retry_backoff', '30');

		return uci.save().then(function() {
			ui.addNotification(null, E('p', _('Rule has been staged. Reloading rules page...')), 'info');
			window.location.reload();
		}).catch(L.bind(function(err) {
			button.disabled = false;
			button.classList.remove('qddns-busy');
			qddns.showFailureModal(_('Add DDNS rule'), { error: qddns.extractResultMessage(err, _('Unable to add the DDNS rule.')) }, _('Unable to add the DDNS rule.'));
		}, this));
	},

	showRuleWizardModal: function(data, launcher) {
		const providers = qddns.sortNamedItems(data?.catalog?.rules?.providers || []);
		const sources = qddns.sortNamedItems(data?.catalog?.sources || []);
		const viewRef = this;
		const fields = {
			recordType: E('select', { class: 'cbi-input-select' }, [
				E('option', { value: 'A' }, ['A']),
				E('option', { value: 'AAAA' }, ['AAAA'])
			]),
			provider: this.renderWizardSelect(providers, _('No providers available')),
			source: this.renderWizardSelect(sources, _('No sources available')),
			zone: E('input', { type: 'text', class: 'cbi-input-text', placeholder: 'example.com' }),
			recordName: E('input', { type: 'text', class: 'cbi-input-text', placeholder: 'home' }),
			enabled: E('input', { type: 'checkbox', checked: 'checked' })
		};
		const sourceIpStatus = E('span', { class: 'qddns-rule-wizard-source-ip', 'data-source-ip-status': 'wizard' }, [_('Loading...')]);
		const sourceProbe = { token: 0, address: '', family: '', detail: '', loading: false };
		const feedback = E('div', { class: 'cbi-value-description qddns-rule-wizard-feedback' }, _('Choose the source IP first, then choose the DNS location.'));
		const saveButton = E('button', { type: 'button', class: 'btn cbi-button cbi-button-add' }, [_('Add DDNS rule')]);
		const previousButton = E('button', { type: 'button', class: 'btn cbi-button' }, [_('Back')]);
		const nextButton = E('button', { type: 'button', class: 'btn cbi-button cbi-button-action' }, [_('Next')]);
		const summary = E('div', { class: 'qddns-rule-wizard-summary' });
		let stepIndex = 0;
		const modal = E('div', { class: 'qddns-rule-wizard-modal' }, [
			E('div', { class: 'qddns-rule-wizard-steps' }, [
				E('span', { 'data-wizard-step': '0', class: 'qddns-rule-wizard-step is-active' }, _('1. Source')),
				E('span', { 'data-wizard-step': '1', class: 'qddns-rule-wizard-step' }, _('2. DNS')),
				E('span', { 'data-wizard-step': '2', class: 'qddns-rule-wizard-step' }, _('3. Confirm'))
			]),
			E('div', { 'data-wizard-panel': '0' }, [
				E('h4', {}, _('Choose Source IP')),
				E('div', { class: 'qddns-rule-wizard-grid' }, [
					this.renderWizardField(_('Source'), fields.source),
					this.renderWizardField(_('Record type'), fields.recordType),
					this.renderWizardSourceIp(sourceIpStatus)
				])
			]),
			E('div', { 'data-wizard-panel': '1', style: 'display:none' }, [
				E('h4', {}, _('Choose where to update DNS')),
				E('div', { class: 'qddns-rule-wizard-grid' }, [
					this.renderWizardField(_('Provider'), fields.provider),
					this.renderWizardField(_('Zone'), fields.zone),
					this.renderWizardField(_('Record name'), fields.recordName)
				])
			]),
			E('div', { 'data-wizard-panel': '2', style: 'display:none' }, [
				E('h4', {}, _('Confirm and create the rule')),
				summary,
				E('p', { class: 'cbi-value-description' }, _('Rule name is generated automatically from the record.')),
				E('div', { class: 'qddns-rule-wizard-grid' }, [
					this.renderWizardField(_('Enable after creation'), E('label', { class: 'qddns-rule-wizard-switch' }, [fields.enabled, _('Enabled')]))
				])
			]),
			feedback,
			E('div', { class: 'qddns-actions' }, [previousButton, nextButton, saveButton, E('button', { type: 'button', class: 'btn cbi-button', click: ui.hideModal }, [_('Close')])])
		]);

		function setWizardSourceIp(message, tone) {
			sourceIpStatus.textContent = message || _('N/A');
			sourceIpStatus.setAttribute('data-tone', tone || 'neutral');
		}

		function updateWizardSummary() {
			summary.replaceChildren(
				E('p', {}, '%s: %s.%s (%s)'.format(_('Record'), viewRef.wizardValue(fields.recordName) || '-', viewRef.wizardValue(fields.zone) || '-', viewRef.wizardValue(fields.recordType) || 'A')),
				E('p', {}, '%s: %s'.format(_('Source'), viewRef.wizardSelectedText(fields.source, _('Unnamed source')))),
				E('p', {}, '%s: %s'.format(_('Source IP'), sourceProbe.address || sourceIpStatus.textContent || _('N/A'))),
				E('p', {}, '%s: %s'.format(_('Provider'), viewRef.wizardSelectedText(fields.provider, _('Unnamed provider'))))
			);
		}

		function updateWizardSourceProbe() {
			const sourceId = viewRef.wizardValue(fields.source);
			sourceProbe.token++;
			const token = sourceProbe.token;
			sourceProbe.address = '';
			sourceProbe.family = '';
			sourceProbe.detail = '';
			sourceProbe.loading = false;
			fields.source.removeAttribute('data-probed-family');
			fields.source.removeAttribute('data-source-ip-loading');

			if (!sourceId) {
				setWizardSourceIp(_('N/A'), 'neutral');
				if (stepIndex === 2)
					updateWizardSummary();
				return Promise.resolve();
			}

			const source = viewRef.findById(sources, sourceId);
			if (!viewRef.isProbeableSourceType(source?.type)) {
				setWizardSourceIp(_('N/A'), 'neutral');
				if (stepIndex === 2)
					updateWizardSummary();
				return Promise.resolve();
			}

			sourceProbe.loading = true;
			fields.source.setAttribute('data-source-ip-loading', '1');
			setWizardSourceIp(_('Loading...'), 'neutral');
			if (stepIndex === 2)
				updateWizardSummary();

			return qddns.probeSource(sourceId).then(function(result) {
				if (token !== sourceProbe.token)
					return result;

				sourceProbe.loading = false;
				fields.source.removeAttribute('data-source-ip-loading');
				if (qddns.isFailedResult(result) || !result.address) {
					setWizardSourceIp(_('Unable to read source IP.'), 'negative');
					if (stepIndex === 2)
						updateWizardSummary();
					return result;
				}

				sourceProbe.address = result.address;
				sourceProbe.family = result.family || '';
				sourceProbe.detail = result.detail || '';
				fields.source.setAttribute('data-probed-family', sourceProbe.family);
				setWizardSourceIp(result.address, 'neutral');
				if (stepIndex === 2)
					updateWizardSummary();
				return result;
			}).catch(function(err) {
				if (token !== sourceProbe.token)
					return;

				sourceProbe.address = '';
				sourceProbe.family = '';
				sourceProbe.detail = qddns.extractResultMessage(err, _('Unable to read source IP.'));
				sourceProbe.loading = false;
				fields.source.removeAttribute('data-probed-family');
				fields.source.removeAttribute('data-source-ip-loading');
				setWizardSourceIp(sourceProbe.detail, 'negative');
				if (stepIndex === 2)
					updateWizardSummary();
			});
		}

		function updateButtons() {
			previousButton.style.display = stepIndex ? '' : 'none';
			nextButton.style.display = stepIndex < 2 ? '' : 'none';
			saveButton.style.display = stepIndex === 2 ? '' : 'none';
			if (stepIndex === 2)
				updateWizardSummary();
			viewRef.setWizardStep(modal, stepIndex);
		}

		previousButton.addEventListener('click', L.bind(function() {
			stepIndex = Math.max(0, stepIndex - 1);
			this.resetWizardFeedback(feedback);
			updateButtons();
		}, this));

		fields.source.addEventListener('change', L.bind(function() {
			this.resetWizardFeedback(feedback);
			updateWizardSourceProbe();
		}, this));

		fields.recordType.addEventListener('change', L.bind(function() {
			this.resetWizardFeedback(feedback);
			if (stepIndex === 2)
				updateWizardSummary();
		}, this));

		nextButton.addEventListener('click', L.bind(function() {
			if (!this.validateWizardStep(fields, feedback, stepIndex))
				return;

			stepIndex = Math.min(2, stepIndex + 1);
			updateButtons();
		}, this));

		saveButton.addEventListener('click', L.bind(function() {
			if (!this.validateWizardStep(fields, feedback, stepIndex))
				return Promise.resolve();

			return this.createRuleFromWizard(fields, feedback, saveButton);
		}, this));

		updateButtons();
		updateWizardSourceProbe();
		ui.showModal(_('Guided DDNS rule setup'), [modal]);

		if (launcher)
			launcher.blur();
		if (fields.source && typeof fields.source.focus == 'function')
			window.setTimeout(function() { fields.source.focus(); }, 0);
	},

	renderRuleWizard: function(data) {
		const button = E('button', { type: 'button', class: 'btn cbi-button cbi-button-add qddns-rule-wizard-primary' }, [_('Start guided setup')]);

		button.addEventListener('click', L.bind(function() {
			this.showRuleWizardModal(data, button);
		}, this));

		return E('div', { id: 'qddns-rule-wizard', class: 'cbi-section qddns-panel' }, [
			E('div', { class: 'qddns-rule-wizard-entry' }, [
				E('div', { class: 'qddns-rule-wizard-entry-text' }, [
					E('h3', {}, _('Guided DDNS rule setup')),
					E('p', { class: 'cbi-section-descr' }, _('Start a short wizard that creates a complete rule with safe defaults. Use the advanced table below for later edits.'))
				]),
				button
			]),
		]);
	},

	useNameColumnHeader: function(section) {
		const renderHeaderRows = section.renderHeaderRows;

		section.renderHeaderRows = function() {
			const rows = renderHeaderRows.apply(this, arguments);
			const nameHeader = rows.querySelector('tr.cbi-section-table-titles th');

			if (nameHeader)
				nameHeader.textContent = _('Name');

			return rows;
		};
	},

	useRuleEditorLabels: function(section) {
		section.sectiontitle = function(sectionId) {
			return uci.get('qddns', sectionId, 'name') || _('Unnamed rule');
		};
		section.modaltitle = section.sectiontitle;
		section.renderSectionAdd = function() {
			return E([]);
		};
		this.useNameColumnHeader(section);
	},


	replaceRulesConsole: function() {
		const root = document.getElementById('qddns-rules-console');
		if (root)
			root.replaceWith(this.renderRulesConsole(this.pageData));
	},

	refreshRuntime: function() {
		return this.loadRuntimeState().then(L.bind(function(runtime) {
			this.pageData = this.buildPageData(runtime, this.pageData?.catalog);
			this.replaceRulesConsole();
			return this.pageData;
		}, this));
	},

	showActionResult: function(title, rule, result) {
		qddns.showInfoModal(title, [
			E('div', { class: 'qddns-modal-meta' }, [
				E('p', {}, '%s: %s'.format(_('Rule'), this.getRuleLabel(rule))),
				E('p', {}, '%s: %s'.format(_('Status'), result.status || _('Unknown'))),
				E('p', {}, '%s: %s'.format(_('Current IP'), result.current_ip || _('N/A'))),
				E('p', {}, '%s: %s'.format(_('Remote IP'), result.remote_ip || _('N/A'))),
				E('p', {}, '%s: %s'.format(_('Changed'), result.changed ? _('Yes') : _('No'))),
				E('p', {}, '%s: %s'.format(_('Detail'), result.detail || _('N/A')))
			])
		]);
	},

	renderRulesConsole: function(data) {
		const ruleStates = this.getRuleStates();
		const rules = qddns.sortNamedItems(data?.catalog?.rules?.rules || []);
		const table = qddns.renderTable([
			_('Rule'), _('Type'), _('Record'), _('Source'), _('Provider'), _('Runtime'), _('Actions')
		], rules.map(L.bind(function(rule) {
			const state = ruleStates[rule.id] || {};
			const runtime = state.status || (rule.enabled ? _('Enabled') : _('Disabled'));

			return [
				this.getRuleLabel(rule),
				rule.record_type || '-',
				'%s.%s'.format(rule.record_name || '-', rule.zone || '-'),
				this.getSourceLabel(rule.source),
				this.getProviderLabel(rule.provider),
				qddns.renderStatusBadge(runtime, _('Unknown')),
				this.renderRuleActions(rule)
			];
		}, this)), _('No rules configured'));

		table.classList.add('qddns-rules-console-table');

		return E('div', { id: 'qddns-rules-console', class: 'cbi-section qddns-panel' }, [
			E('h3', {}, _('Rule Console')),
			E('p', { class: 'cbi-section-descr' }, _('Run and test saved rules here. Runtime status comes from the live overview state; provider and source references come from saved providers and sources, so save and reload after changing related settings.')),
			table
		]);
	},

	renderRuleActions: function(rule) {
		const wrap = E('div', { class: 'qddns-actions' });
		const ruleLabel = this.getRuleLabel(rule);
		const actions = [
			{ label: _('Run'), handler: qddns.runRule, title: _('Run Rule'), fallback: _('Unable to run the selected rule.') },
			{ label: _('Test'), handler: qddns.testRule, title: _('Test Rule'), fallback: _('Unable to test the selected rule.') }
		];

		actions.forEach(L.bind(function(action) {
			const button = E('button', { class: 'btn cbi-button cbi-button-action' }, [action.label]);
			button.addEventListener('click', L.bind(function() {
				return qddns.handleMutationAction(button, action.title, function() {
					return action.handler(rule.id);
				}, L.bind(function(result) {
					this.showActionResult(action.title, rule, result);
				}, this), action.fallback, L.bind(this.refreshRuntime, this));
			}, this));
			wrap.appendChild(button);
		}, this));

		const statusButton = E('button', { class: 'btn cbi-button' }, [_('Status')]);
		statusButton.addEventListener('click', L.bind(function() {
			return qddns.handleReadAction(statusButton, _('Rule Status'), function() {
				return qddns.getRuleStatus(rule.id);
			}, function(result) {
				qddns.showInfoModal(_('Rule Status'), [
					E('h4', {}, ruleLabel),
					E('div', { class: 'qddns-modal-meta' }, [
						E('p', {}, '%s: %s'.format(_('Daemon'), result.running ? _('Running') : _('Stopped'))),
						E('p', {}, '%s: %s'.format(_('Status'), result.status || _('Unknown'))),
						E('p', {}, '%s: %s'.format(_('Current IP'), result.current_ip || _('N/A'))),
						E('p', {}, '%s: %s'.format(_('Remote IP'), result.remote_ip || _('N/A'))),
						E('p', {}, '%s: %s'.format(_('Last Result'), result.last_result || _('N/A'))),
						E('p', {}, '%s: %s'.format(_('Last Error'), result.last_error || _('N/A'))),
						E('p', {}, '%s: %s'.format(_('Last Check'), result.last_check ? qddns.formatEpoch(result.last_check) : _('N/A'))),
						E('p', {}, '%s: %s'.format(_('Next Run'), result.next_run ? qddns.formatEpoch(result.next_run) : _('N/A')))
					])
				]);
			}, _('Unable to load rule status.'));
		}, this));
		wrap.appendChild(statusButton);

		return wrap;
	},

	renderRuleForm: function(data) {
		const providers = qddns.sortNamedItems(data?.catalog?.rules?.providers || []);
		const sources = qddns.sortNamedItems(data?.catalog?.sources || []);
		const m = new form.Map('qddns', _('QDDNS'), _('Only rules are editable on this page. Providers and sources live on the settings page.'));
		let s;
		let o;

		s = m.section(form.GridSection, 'rule', _('Rule references use the latest saved providers and sources loaded with this page. Save and reload after adding referenced providers or sources on the settings page.'));
		s.addremove = true;
		s.anonymous = false;
		this.useRuleEditorLabels(s);

		o = s.option(form.Value, 'name', _('Name'), _('Name shown in the rule table, console, and log selector.'));
		o.placeholder = _('Unnamed rule');
		o.modalonly = true;
		o = s.option(form.Flag, 'enabled', _('Enabled'));
		o.rmempty = false;
		o = s.option(form.ListValue, 'record_type', _('Record type'));
		o.value('A');
		o.value('AAAA');
		o = s.option(form.ListValue, 'provider', _('Provider'));
		providers.forEach(function(provider) { o.value(provider.id, provider.name || _('Unnamed provider')); });
		o.modalonly = true;
		o = s.option(form.ListValue, 'source', _('Source'));
		sources.forEach(function(source) { o.value(source.id, source.name || _('Unnamed source')); });
		o.modalonly = true;
		o = s.option(form.Value, 'zone', _('Zone'));
		o.placeholder = 'example.com';
		o.modalonly = true;
		o = s.option(form.Value, 'record_name', _('Record name'));
		o.placeholder = 'home';
		o.modalonly = true;
		o = s.option(form.Value, 'ttl', _('TTL'));
		o.datatype = 'uinteger';
		o.modalonly = true;
		o = s.option(form.Flag, 'proxied', _('Proxied'));
		o.modalonly = true;
		o = s.option(form.Value, 'check_interval', _('Check interval'));
		o.datatype = 'uinteger';
		o.modalonly = true;
		o = s.option(form.Value, 'force_interval', _('Force interval'));
		o.datatype = 'uinteger';
		o.modalonly = true;
		o = s.option(form.Value, 'retry_count', _('Retry count'));
		o.datatype = 'uinteger';
		o.modalonly = true;
		o = s.option(form.Value, 'retry_backoff', _('Retry backoff'));
		o.datatype = 'uinteger';
		o.modalonly = true;

		return m.render();
	},

	render: function(data) {
		this.pageData = data || this.buildPageData({}, {});
		this.ensurePageStyle();

		return this.renderRuleForm(this.pageData).then(L.bind(function(formEl) {
			return E('div', { class: 'qddns-rules-page' }, [
				this.renderRulesConsole(this.pageData),
				this.renderRuleWizard(this.pageData),
				E('div', { class: 'qddns-wide-form qddns-rules-form' }, [formEl])
			]);
		}, this));
	}
});
