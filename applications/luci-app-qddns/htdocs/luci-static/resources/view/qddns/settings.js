'use strict';
'require ui';
'require uci';
'require view';
'require form';
'require tools.widgets as widgets';
'require view.qddns.shared as qddns';

const PROVIDER_TEMPLATES = {
	cloudflare: {
		label: _('Cloudflare'),
		values: {
			name: _('Cloudflare'),
			type: 'cloudflare'
		}
	},
	dnspod: {
		label: _('DNSPod'),
		values: {
			name: _('DNSPod'),
			type: 'dnspod'
		}
	},
	aliyun: {
		label: _('Aliyun'),
		values: {
			name: _('Aliyun'),
			type: 'aliyun'
		}
	},
	custom_http: {
		label: _('Custom HTTP'),
		values: {
			name: _('Custom HTTP'),
			type: 'custom_http',
			method: 'POST',
			url: 'https://api.example.com/ddns/update',
			headers_json: '{"Content-Type":"application/json"}',
			body_template: '{"zone":"{{zone}}","record":"{{record_name}}","type":"{{record_type}}","ip":"{{ip}}"}',
			success_contains: 'ok'
		}
	}
};

const QDDNS_SETTINGS_STYLE_ID = 'qddns-settings-style';
const SOURCE_OPTION_FIELDS = ['family', 'address', 'interface', 'duid', 'iaid', 'mac', 'lease_file', 'hostname_hint', 'prefix_filter', 'probe_url', 'script'];
const SOURCE_FIELDS_BY_TYPE = {
	local_addr: ['family', 'address'],
	interface: ['family', 'interface'],
	public_probe: ['family', 'probe_url'],
	script: ['family', 'script'],
	dhcpv6_duid: ['interface', 'duid', 'iaid', 'lease_file', 'hostname_hint', 'prefix_filter'],
	dhcpv6_mac: ['interface', 'mac', 'lease_file', 'hostname_hint', 'prefix_filter']
};
const QDDNS_SETTINGS_STYLE = [
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
	'}',
	'.qddns-settings-page{margin-bottom:var(--qddns-space-4)}',
	'.qddns-settings-page .qddns-panel{margin-bottom:var(--qddns-space-4);padding:var(--qddns-space-4);border:1px solid var(--qddns-border);border-radius:var(--qddns-radius-md);background:var(--qddns-surface)}',
	'.qddns-actions{display:flex;flex-wrap:wrap;gap:var(--qddns-space-2);max-width:100%}',
	'.qddns-actions .cbi-button{margin:0;max-width:100%;white-space:normal}',
	'.qddns-actions .cbi-button.qddns-busy{opacity:0.7;cursor:progress}',
	'.qddns-dhcpv6-lease-status{display:grid;gap:var(--qddns-space-2);width:100%;max-width:100%;min-width:0;text-align:left}',
	'.qddns-dhcpv6-lease-results{display:grid;gap:var(--qddns-space-2);width:100%;max-width:100%;min-width:0;text-align:left}',
	'.qddns-dhcpv6-lease-list{display:grid;justify-items:stretch;gap:var(--qddns-space-2);width:100%;max-width:100%;min-width:0}',
	'.qddns-dhcpv6-lease-card{appearance:none;box-sizing:border-box;display:grid;align-items:start;justify-items:stretch;justify-content:stretch;gap:var(--qddns-space-2);width:100%!important;min-width:0;margin:0;padding:var(--qddns-space-3);border:1px solid var(--qddns-border);border-radius:var(--qddns-radius-sm);background:var(--qddns-surface);color:inherit;font:inherit;line-height:1.35;text-align:left!important;text-transform:none;cursor:pointer}',
	'.qddns-dhcpv6-lease-card:hover,.qddns-dhcpv6-lease-card:focus,.qddns-dhcpv6-lease-card.is-selected{border-color:currentColor;background:var(--qddns-surface-strong)}',
	'.qddns-dhcpv6-lease-card-head{display:grid;grid-template-columns:minmax(0,1fr) auto;align-items:start;gap:var(--qddns-space-2);width:100%;justify-self:stretch;min-width:0;text-align:left}',
	'.qddns-dhcpv6-lease-title{justify-self:start;min-width:0;font-weight:600;text-align:left;overflow-wrap:anywhere}',
	'.qddns-dhcpv6-lease-action{justify-self:end;max-width:100%;padding:0.1rem 0.4rem;border-radius:999px;background:var(--qddns-surface-strong);font-size:0.9em;line-height:1.35;opacity:0.85;text-align:center;white-space:nowrap}',
	'.qddns-dhcpv6-lease-meta{display:grid;grid-template-columns:1fr;gap:var(--qddns-space-1);width:100%;justify-self:stretch;min-width:0;text-align:left}',
	'.qddns-dhcpv6-lease-meta-item{display:grid;grid-template-columns:minmax(4.75rem,max-content) minmax(0,1fr);gap:var(--qddns-space-1);width:100%;justify-self:stretch;min-width:0;text-align:left;overflow-wrap:break-word;word-break:normal}',
	'.qddns-dhcpv6-lease-meta-label{min-width:4.75rem;opacity:0.72}',
	'.qddns-dhcpv6-lease-meta-value{min-width:0;overflow-wrap:anywhere;word-break:normal;white-space:pre-wrap;text-align:left}',
	'.qddns-source-ip-probe{display:flex;flex-wrap:wrap;align-items:center;gap:var(--qddns-space-2);max-width:100%;min-width:0}',
	'.qddns-source-ip-probe .cbi-button{margin:0}',
	'.qddns-source-ip-status{display:block;max-width:100%;min-width:min(100%,8rem);overflow-wrap:anywhere;text-align:left}',
	'.qddns-source-ip-status[data-tone="warning"]{opacity:0.78}',
	'.qddns-source-ip-status[data-tone="negative"]{color:var(--qddns-negative-text,inherit)}',
	'@media (max-width: 768px){',
		'.qddns-settings-page .qddns-panel{padding:var(--qddns-space-3)}',
	'}'
].join('');

return view.extend({
	load: function() {
		return Promise.all([
			L.resolveDefault(qddns.listRules(), { providers: [], rules: [] }),
			L.resolveDefault(qddns.listSources(), { result: [] }),
			uci.load('qddns')
		]).then(function(data) {
			return qddns.normalizeCatalogState(data[0], data[1]);
		});
	},

	ensureSettingsStyle: function() {
		if (document.getElementById(QDDNS_SETTINGS_STYLE_ID))
			return;

		document.head.appendChild(E('style', { id: QDDNS_SETTINGS_STYLE_ID }, [QDDNS_SETTINGS_STYLE]));
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

	validateVisibleName: function(name, emptyMessage) {
		if (!name)
			return emptyMessage;

		return null;
	},

	getDhcpv6OptionSet: function(section) {
		const options = Object.assign({}, this.sourceDhcpv6Options || {});
		const children = section?.children || [];

		children.forEach(function(option) {
			switch (option.option) {
			case 'type':
				options.type = option;
				break;
			case 'family':
				options.family = option;
				break;
			case 'address':
				options.address = option;
				break;
			case 'duid':
				options.duid = option;
				break;
			case 'mac':
				options.mac = option;
				break;
			case 'iaid':
				options.iaid = option;
				break;
			case 'lease_file':
				options.leaseFile = option;
				break;
			case 'hostname_hint':
				options.hostnameHint = option;
				break;
			case 'prefix_filter':
				options.prefixFilter = option;
				break;
			case 'interface':
				options.interface = option;
				break;
			}
		});

		return options;
	},

	getSourceOptionValue: function(option, sectionId) {
		if (!option || typeof option.getUIElement != 'function' || !option.map?.root)
			return '';

		const widget = option.getUIElement(sectionId);
		if (!widget)
			return '';

		if (typeof widget.getValue == 'function') {
			const value = widget.getValue();
			return Array.isArray(value) ? value.join(',') : String(value || '');
		}

		const input = widget.node?.querySelector('input,select,textarea');
		return input ? String(input.value || '') : '';
	},

	interfaceValues: function(value) {
		return L.toArray(value).flatMap(function(item) {
			return String(item || '').split(/,+/);
		}).map(function(item) {
			return item.trim();
		}).filter(function(item, index, values) {
			return item && values.indexOf(item) === index;
		});
	},

	sourceFieldsForType: function(sourceType) {
		return SOURCE_FIELDS_BY_TYPE[sourceType] || [];
	},

	isSourceFieldActive: function(field, sourceType) {
		return this.sourceFieldsForType(sourceType).indexOf(field) > -1;
	},

	cleanupSourceTypeOptions: function(sectionId, sourceType) {
		const fields = this.sourceFieldsForType(sourceType);

		SOURCE_OPTION_FIELDS.forEach(function(field) {
			if (fields.indexOf(field) === -1)
				uci.unset('qddns', sectionId, field);
		});
	},

	guardSourceOptionWrite: function(option, field) {
		const viewRef = this;
		const write = option.write;

		option.write = function(sectionId, value) {
			const sourceType = uci.get('qddns', sectionId, 'type') || viewRef.getSourceOptionValue(viewRef.sourceDhcpv6Options?.type, sectionId);

			if (!viewRef.isSourceFieldActive(field, sourceType)) {
				uci.unset('qddns', sectionId, field);
				return;
			}

			return write.apply(this, arguments);
		};
	},

	getSourceType: function(sectionId, optionSet) {
		return this.getSourceOptionValue(optionSet?.type, sectionId) || uci.get('qddns', sectionId, 'type') || '';
	},

	isProbeableSourceType: function(sourceType) {
		return ['local_addr', 'interface', 'dhcpv6_duid', 'dhcpv6_mac'].indexOf(sourceType) > -1;
	},

	setSourceIpStatus: function(node, message, tone) {
		node.textContent = message || _('N/A');
		node.setAttribute('data-tone', tone || 'neutral');
	},

	bindSourceOptionChange: function(sectionId, optionSet, handler, optionNames) {
		const names = optionNames || ['type', 'family', 'address', 'duid', 'iaid', 'mac', 'leaseFile', 'prefixFilter', 'interface'];

		window.setTimeout(function() {
			names.forEach(function(name) {
				const option = optionSet?.[name];
				if (!option || typeof option.getUIElement != 'function')
					return;

				const widget = option.getUIElement(sectionId);
				const node = widget?.node;
				const input = node?.querySelector('input,select,textarea') || node;
				if (!input || typeof input.addEventListener != 'function')
					return;

				input.addEventListener('input', handler);
				input.addEventListener('change', handler);
			});
		}, 0);
	},

	renderSourceIpStatus: function(sectionId, optionSet) {
		const node = E('span', { class: 'qddns-source-ip-status', 'data-source-ip-status': sectionId }, [_('N/A')]);
		const probeButton = E('button', { type: 'button', class: 'btn cbi-button cbi-button-action' }, [_('Probe')]);
		const sourceIpProbe = { token: 0 };

		this.bindSourceOptionChange(sectionId, optionSet, L.bind(function() {
			sourceIpProbe.token++;
			this.setSourceIpStatus(node, _('Save and reload to read updated source IP.'), 'warning');
		}, this));

		probeButton.addEventListener('click', L.bind(function() {
			return qddns.withBusyButton(probeButton, L.bind(function() {
				return this.updateSourceIpStatus(sectionId, optionSet, node, sourceIpProbe);
			}, this));
		}, this));

		return E('span', { class: 'qddns-source-ip-probe' }, [node, probeButton]);
	},

	updateSourceIpStatus: function(sectionId, optionSet, node, sourceIpProbe) {
		const sourceType = this.getSourceType(sectionId, optionSet);
		sourceIpProbe.token++;
		const token = sourceIpProbe.token;

		if (!this.isProbeableSourceType(sourceType)) {
			this.setSourceIpStatus(node, _('N/A'), 'neutral');
			return Promise.resolve();
		}

		this.setSourceIpStatus(node, _('Loading...'), 'neutral');

		return qddns.probeSource(sectionId).then(L.bind(function(result) {
			if (token !== sourceIpProbe.token)
				return result;

			if (qddns.isFailedResult(result) || !result.address) {
				this.setSourceIpStatus(node, _('Unable to read source IP.'), 'negative');
				return result;
			}

			this.setSourceIpStatus(node, result.address, 'neutral');
			return result;
		}, this)).catch(L.bind(function(err) {
			if (token !== sourceIpProbe.token)
				return;

			this.setSourceIpStatus(node, qddns.extractResultMessage(err, _('Unable to read source IP.')), 'negative');
		}, this));
	},

	isDhcpv6DuidSource: function(sectionId, optionSet) {
		return this.getSourceType(sectionId, optionSet) === 'dhcpv6_duid';
	},

	getDhcpv6LeaseMode: function(sectionId, optionSet) {
		return this.isDhcpv6DuidSource(sectionId, optionSet) ? 'duid' : 'mac';
	},

	setSourceOptionValue: function(option, sectionId, value) {
		if (!option)
			return;

		const widget = option.getUIElement(sectionId);
		if (!widget || typeof widget.setValue != 'function')
			return;

		const normalized = option.multiple ? this.interfaceValues(value) : (value == null ? '' : String(value));
		widget.setValue(normalized);

		if (widget.node) {
			widget.node.setAttribute('data-changed', 'true');

			const input = widget.node.querySelector('input,select,textarea') || widget.node;
			input.dispatchEvent(new Event('input', { bubbles: true }));
			input.dispatchEvent(new Event('change', { bubbles: true }));
			widget.node.dispatchEvent(new CustomEvent('widget-update', { bubbles: true }));
			widget.node.dispatchEvent(new CustomEvent('widget-change', { bubbles: true }));
		}

		if (typeof option.triggerValidation == 'function')
			option.triggerValidation(sectionId);
	},

	fillDhcpv6Lease: function(sectionId, lease, feedback, optionSet) {
		const options = optionSet || this.sourceDhcpv6Options || {};
		const isDuidSource = this.isDhcpv6DuidSource(sectionId, options);

		this.setSourceOptionValue(options.family, sectionId, 'ipv6');
		if (isDuidSource) {
			this.setSourceOptionValue(options.duid, sectionId, lease?.duid || '');
			this.setSourceOptionValue(options.iaid, sectionId, lease?.iaid || '');
		} else {
			this.setSourceOptionValue(options.mac, sectionId, lease?.mac || '');
		}
		this.setSourceOptionValue(options.leaseFile, sectionId, lease?.lease_file || '/tmp/odhcpd.leases');
		this.setSourceOptionValue(options.hostnameHint, sectionId, lease?.hostname || '');
		this.setSourceOptionValue(options.interface, sectionId, lease?.interface || '');
		this.setSourceOptionValue(options.prefixFilter, sectionId, '');

		if (feedback)
			feedback.textContent = isDuidSource ? _('Selected DHCPv6 lease values have been filled. Save the source to keep them.') : _('Selected LAN host MAC has been filled. Save the source to keep it.');
	},

	filterDhcpv6Choices: function(sectionId, leases, optionSet) {
		const isDuidSource = this.isDhcpv6DuidSource(sectionId, optionSet);

		return qddns.normalizeList(leases).filter(function(lease) {
			const prefixes = qddns.normalizeList(lease?.prefixes);

			if (!prefixes.length)
				return false;

			return isDuidSource ? !!(lease?.duid && lease?.iaid) : !!lease?.mac;
		});
	},

	renderDhcpv6LeaseStatus: function(sectionId, optionSet) {
		this.ensureSettingsStyle();

		const loadButton = E('button', { type: 'button', class: 'btn cbi-button cbi-button-action' });
		const results = E('div', { class: 'qddns-dhcpv6-lease-results' });
		const resetResults = L.bind(function() {
			const isDuidSource = this.isDhcpv6DuidSource(sectionId, optionSet);

			loadButton.replaceChildren(isDuidSource ? _('Read current DUID') : _('Read current MAC'));
			results.replaceChildren(E('div', { class: 'cbi-value-description' }, isDuidSource ? _('Read current DHCPv6 lease candidates, then choose one to fill the DUID source fields.') : _('Read current LAN host candidates, then choose one to fill the MAC source fields.')));
		}, this);

		resetResults();
		this.bindSourceOptionChange(sectionId, optionSet, resetResults, ['type']);

		loadButton.addEventListener('click', L.bind(function(ev) {
			return this.handleDhcpv6LeaseLoad(ev, sectionId, results, optionSet);
		}, this));

		return E('div', { class: 'qddns-dhcpv6-lease-status' }, [
			E('div', { class: 'qddns-actions' }, [loadButton]),
			results
		]);
	},

	renderDhcpv6LeaseMeta: function(label, value) {
		return E('span', { class: 'qddns-dhcpv6-lease-meta-item' }, [
			E('span', { class: 'qddns-dhcpv6-lease-meta-label' }, label + ': '),
			E('span', { class: 'qddns-dhcpv6-lease-meta-value' }, value || '-')
		]);
	},

	renderDhcpv6LeaseCard: function(sectionId, lease, feedback, optionSet) {
		const prefixes = qddns.normalizeList(lease?.prefixes);
		const ipv4 = qddns.normalizeList(lease?.ipv4);
		const isDuidSource = this.isDhcpv6DuidSource(sectionId, optionSet);
		const identityMeta = isDuidSource ? [
			this.renderDhcpv6LeaseMeta(_('DUID'), lease?.duid || '-'),
			this.renderDhcpv6LeaseMeta(_('IAID'), lease?.iaid || '-')
		] : [
			this.renderDhcpv6LeaseMeta(_('MAC'), lease?.mac || '-'),
			this.renderDhcpv6LeaseMeta(_('LAN IP'), ipv4.length ? ipv4.join(', ') : '-')
		];
		const card = E('button', {
			type: 'button',
			class: 'qddns-dhcpv6-lease-card',
			'aria-pressed': 'false',
			title: _('Fill from this lease')
			}, [
				E('span', { class: 'qddns-dhcpv6-lease-card-head' }, [
					E('span', { class: 'qddns-dhcpv6-lease-title' }, lease?.hostname || _('Unnamed host')),
					E('span', { class: 'qddns-dhcpv6-lease-action' }, _('Fill from this lease'))
				]),
				E('span', { class: 'qddns-dhcpv6-lease-meta' }, identityMeta.concat([
					this.renderDhcpv6LeaseMeta(_('Prefix'), prefixes.length ? prefixes.join('\n') : '-'),
					this.renderDhcpv6LeaseMeta(_('Interface'), lease?.interface || '-')
				]))
			]);

		card.addEventListener('click', L.bind(function() {
			const selected = card.parentNode?.querySelector('.qddns-dhcpv6-lease-card.is-selected');

			if (selected) {
				selected.classList.remove('is-selected');
				selected.setAttribute('aria-pressed', 'false');
			}

			card.classList.add('is-selected');
			card.setAttribute('aria-pressed', 'true');
			this.fillDhcpv6Lease(sectionId, lease, feedback, optionSet);
		}, this));

		return card;
	},

	renderDhcpv6LeaseResults: function(sectionId, leases, optionSet) {
		const list = this.filterDhcpv6Choices(sectionId, leases, optionSet);
		const isDuidSource = this.isDhcpv6DuidSource(sectionId, optionSet);
		const emptyMessage = isDuidSource ? _('No DHCPv6 leases found.') : _('No LAN hosts with public IPv6 found.');
		const feedback = E('div', { class: 'cbi-value-description' }, list.length ? (isDuidSource ? _('Choose a current DUID to fill DUID, IAID, interface, and hostname hint.') : _('Choose a current MAC to fill MAC, LAN IP identity, interface, and hostname hint.')) : emptyMessage);

		if (!list.length)
			return E('div', { class: 'qddns-dhcpv6-lease-results' }, [feedback]);

		return E('div', { class: 'qddns-dhcpv6-lease-results' }, [
			feedback,
			E('div', { class: 'qddns-dhcpv6-lease-list' }, list.map(L.bind(function(lease) {
				return this.renderDhcpv6LeaseCard(sectionId, lease, feedback, optionSet);
			}, this)))
		]);
	},

	showDhcpv6LeaseResults: function(anchor, node, target) {
		if (target) {
			target.replaceChildren.apply(target, Array.from(node.childNodes));
			return;
		}

		const field = target || anchor?.closest('.cbi-value-field') || anchor?.parentNode;
		if (!field)
			return;

		const existing = field.querySelector('.qddns-dhcpv6-lease-results');
		if (existing)
			existing.remove();

		field.appendChild(node);
	},

	handleDhcpv6LeaseLoad: function(ev, sectionId, target, optionSet) {
		const button = ev.currentTarget;
		const title = this.isDhcpv6DuidSource(sectionId, optionSet) ? _('DHCPv6 leases') : _('LAN hosts');

		return qddns.withBusyButton(button, L.bind(function() {
			return qddns.listDhcpv6Leases(this.getDhcpv6LeaseMode(sectionId, optionSet)).then(L.bind(function(result) {
				if (qddns.isFailedResult(result)) {
					qddns.showFailureModal(title, result, _('Unable to load host candidates.'));
					return result;
				}

				this.showDhcpv6LeaseResults(button, this.renderDhcpv6LeaseResults(sectionId, result.leases, optionSet), target);
				return result;
			}, this)).catch(function(err) {
				qddns.showFailureModal(title, { error: qddns.extractResultMessage(err, _('Unable to load host candidates.')) }, _('Unable to load host candidates.'));
			});
		}, this));
	},

	getSourceLabel: function(source) {
		return source?.name || _('Unnamed source');
	},

	getProviderLabel: function(provider) {
		return provider?.name || _('Unnamed provider');
	},

	createProviderFromTemplate: function(providerName, templateId) {
		const template = PROVIDER_TEMPLATES[templateId] || PROVIDER_TEMPLATES.custom_http;
		const sectionId = uci.add('qddns', 'provider', this.nextNumericSectionId());

		Object.keys(template.values).forEach(function(key) {
			uci.set('qddns', sectionId, key, template.values[key]);
		});
		uci.set('qddns', sectionId, 'name', providerName);

		return uci.save().then(function() {
			ui.addNotification(null, E('p', _('Provider template has been staged. Reloading settings page...')), 'info');
			window.location.reload();
		});
	},

	renderProviderTemplatePanel: function() {
		const nameInput = E('input', {
			type: 'text',
			class: 'cbi-input-text',
			placeholder: _('Provider name')
		});
		const templateSelect = E('select', { class: 'cbi-input-select' });
		const feedback = E('div', { class: 'cbi-value-description' }, _('This name is shown in provider tables and rule selectors.'));
		const createButton = E('button', { type: 'button', class: 'btn cbi-button cbi-button-add' }, [_('Create from template')]);

		Object.keys(PROVIDER_TEMPLATES).forEach(function(templateId) {
			templateSelect.appendChild(E('option', { value: templateId }, [PROVIDER_TEMPLATES[templateId].label]));
		});

		nameInput.addEventListener('input', function() {
			nameInput.classList.remove('cbi-input-invalid');
		});

		createButton.addEventListener('click', L.bind(function() {
			const providerName = nameInput.value.trim();
			const validationError = this.validateVisibleName(providerName, _('Provider name must not be empty.'));

			if (validationError) {
				nameInput.classList.add('cbi-input-invalid');
				feedback.textContent = validationError;
				return;
			}

			createButton.disabled = true;
			createButton.classList.add('qddns-busy');

			return this.createProviderFromTemplate(providerName, templateSelect.value).catch(function(err) {
				createButton.disabled = false;
				createButton.classList.remove('qddns-busy');
				qddns.showFailureModal(_('Provider templates'), { error: qddns.extractResultMessage(err, _('Unable to create provider from template.')) }, _('Unable to create provider from template.'));
			});
		}, this));

		return E('div', { class: 'cbi-section qddns-panel' }, [
			E('h3', {}, _('Provider templates')),
		E('p', { class: 'cbi-section-descr' }, _('Create a new named provider from a safe template. The name can be changed later.')),
			E('p', { class: 'cbi-section-descr' }, _('Copy template values into a new provider without exposing credentials in the main table.')),
			E('div', { class: 'cbi-value' }, [
				E('label', { class: 'cbi-value-title' }, _('Provider name')),
				E('div', { class: 'cbi-value-field' }, [nameInput, feedback])
			]),
			E('div', { class: 'cbi-value' }, [
				E('label', { class: 'cbi-value-title' }, _('Template')),
				E('div', { class: 'cbi-value-field' }, [templateSelect])
			]),
			E('div', { class: 'cbi-value' }, [
				E('label', { class: 'cbi-value-title' }, _('Copy template')),
				E('div', { class: 'cbi-value-field' }, [createButton])
			])
		]);
	},

	renderSourceProbePanel: function(sources) {
		return qddns.renderTableSection(_('Source Probe'), [
			_('Name'), _('Type'), _('Family'), _('Hint'), _('Actions')
		], qddns.normalizeList(sources).map(L.bind(function(src) {
			const probeButton = E('button', { class: 'btn cbi-button cbi-button-action' }, [_('Probe')]);
			const sourceName = this.getSourceLabel(src);

			probeButton.addEventListener('click', function() {
				return qddns.handleReadAction(probeButton, _('Source Probe'), function() {
					return qddns.probeSource(src.id);
				}, function(result) {
					qddns.showInfoModal(_('Source Probe'), [
						E('div', { class: 'qddns-modal-meta' }, [
							E('p', {}, '%s: %s'.format(_('Source'), sourceName)),
							E('p', {}, '%s: %s'.format(_('Address'), result.address || _('N/A'))),
							E('p', {}, '%s: %s'.format(_('Family'), result.family || _('N/A'))),
							E('p', {}, '%s: %s'.format(_('Detail'), result.detail || _('N/A')))
						])
					]);
				}, _('Unable to probe the selected source.'));
			});

			return [
				sourceName,
				src.type || '-',
				src.family || '-',
				src.hint || '-',
				probeButton
			];
		}, this)), _('No sources configured'));
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

	useNameCreateFlow: function(section, options) {
		const viewRef = this;
		const handleAdd = section.handleAdd;

		section.sectiontitle = function(sectionId) {
			return uci.get('qddns', sectionId, 'name') || options.unnamed;
		};
		section.modaltitle = section.sectiontitle;

		section.renderSectionAdd = function(extraClass) {
			if (!this.addremove)
				return E([]);

			const createEl = E('div', { class: 'cbi-section-create' });
			const buttonTitle = this.titleFn('addbtntitle') || _('Add');
			const input = E('input', {
				type: 'text',
				class: 'cbi-input-text',
				placeholder: options.placeholder,
				'aria-label': options.placeholder,
				title: options.placeholder,
				disabled: this.map.readonly || null
			});
			const button = E('button', {
				class: 'cbi-button cbi-button-add',
				title: buttonTitle,
				click: ui.createHandlerFn(this, function(ev) {
					return this.handleAdd(ev, input.value);
				}),
				disabled: this.map.readonly || true
			}, [buttonTitle]);

			if (extraClass != null)
				createEl.classList.add(extraClass);

			input.addEventListener('input', L.bind(function() {
				input.classList.remove('cbi-input-invalid');
				button.disabled = this.map.readonly || !String(input.value || '').trim() ? true : null;
			}, this));

			createEl.appendChild(E('div', {}, [
				input,
				E('div', { class: 'cbi-value-description' }, options.description)
			]));
			createEl.appendChild(button);
			return createEl;
		};

		section.handleAdd = function(ev, name) {
			const visibleName = String(name || '').trim();
			const validationError = viewRef.validateVisibleName(visibleName, options.emptyMessage);

			if (validationError) {
				ui.addNotification(null, E('p', validationError), 'warning');
				return Promise.resolve();
			}

			const configName = this.uciconfig || this.map.config;
			const sectionId = viewRef.nextNumericSectionId();
			const addTask = handleAdd.call(this, ev, sectionId);
			uci.set(configName, sectionId, 'name', visibleName);
			return addTask;
		};

		this.useNameColumnHeader(section);
	},

	renderConfigForms: function(data) {
		this.ensureSettingsStyle();

		const viewRef = this;
		const m = new form.Map('qddns', _('QDDNS'), _('Configure daemon defaults, source definitions, and provider credentials here. Save and reload after adding providers or sources so dependent rules can pick up the latest choices.'));
		let s;
		let o;

		s = m.section(form.NamedSection, 'main', 'qddns', _('Daemon Settings'), _('These values are saved to UCI. Rules on the dedicated rules page reuse the latest saved sources and providers after you save and reload.'));
		o = s.option(form.Flag, 'enabled', _('Enable daemon'));
		o.rmempty = false;

		o = s.option(form.Value, 'log_dir', _('Log directory'));
		o.placeholder = '/var/log/qddns';

		o = s.option(form.Value, 'state_dir', _('State directory'));
		o.placeholder = '/var/run/qddns';

		o = s.option(form.Value, 'poll_interval', _('Poll interval (seconds)'));
		o.datatype = 'range(1, 86400)';

		o = s.option(form.Value, 'timeout', _('Default timeout (seconds)'));
		o.datatype = 'range(1, 30)';

		o = s.option(form.ListValue, 'log_level', _('Log level'));
		['error', 'warn', 'info', 'debug', 'trace'].forEach(function(level) { o.value(level); });

		s = m.section(form.GridSection, 'source', _('Source Library'), _('Saved sources are available for source probe below and become selectable on the rules page after saving and reloading. Names stay editable.'));
		s.addremove = true;
		s.anonymous = false;
		s.nodescriptions = true;
		this.useNameCreateFlow(s, {
			placeholder: _('New source name'),
			description: _('Enter the source name shown in tables and rule selectors.'),
			emptyMessage: _('Source name must not be empty.'),
			unnamed: _('Unnamed source')
		});

		o = s.option(form.Value, 'name', _('Name'), _('Name shown in tables, probes, and rule selectors.'));
		o.placeholder = _('Unnamed source');
		o.modalonly = true;

		o = s.option(form.ListValue, 'type', _('Type'));
		o.value('local_addr', _('Local address'));
		o.value('dhcpv6_duid', _('DHCPv6 DUID'));
		o.value('dhcpv6_mac', _('MAC'));
		o.value('interface', _('Interface'));
		o.value('public_probe', _('Public probe'));
		o.value('script', _('Script'));
		this.sourceDhcpv6Options = {};
		this.sourceDhcpv6Options.type = o;
		const sourceTypeWrite = o.write;
		o.write = function(sectionId, value) {
			const result = sourceTypeWrite.apply(this, arguments);
			viewRef.cleanupSourceTypeOptions(sectionId, value);
			return result;
		};

		o = s.option(form.ListValue, 'family', _('Family'));
		o.value('', _('Auto'));
		o.value('ipv4', _('IPv4'));
		o.value('ipv6', _('IPv6'));
		o.depends('type', 'local_addr');
		o.depends('type', 'interface');
		o.depends('type', 'public_probe');
		o.depends('type', 'script');
		this.sourceDhcpv6Options.family = o;
		this.guardSourceOptionWrite(o, 'family');
		o = s.option(form.Value, 'address', _('Address')); o.modalonly = true; o.depends('type', 'local_addr'); this.sourceDhcpv6Options.address = o; this.guardSourceOptionWrite(o, 'address');
		o = s.option(form.DummyValue, '_source_ip', _('Source IP'));
		o.rawhtml = true;
		o.cfgvalue = function(sectionId) {
			if (arguments.length > 1)
				return null;

			return viewRef.renderSourceIpStatus(sectionId, viewRef.getDhcpv6OptionSet(this.section));
		};
		o = s.option(form.DummyValue, '_dhcpv6_status', _('Status'));
		o.rawhtml = true;
		o.modalonly = true;
		o.depends('type', 'dhcpv6_duid');
		o.depends('type', 'dhcpv6_mac');
		o.cfgvalue = function(sectionId) {
			if (arguments.length > 1)
				return null;

			return viewRef.renderDhcpv6LeaseStatus(sectionId, viewRef.getDhcpv6OptionSet(this.section));
		};
		o = s.option(form.Value, 'duid', _('DUID')); this.sourceDhcpv6Options.duid = o; o.modalonly = true; o.depends('type', 'dhcpv6_duid'); this.guardSourceOptionWrite(o, 'duid');
		o = s.option(form.Value, 'iaid', _('IAID')); this.sourceDhcpv6Options.iaid = o; o.modalonly = true; o.depends('type', 'dhcpv6_duid'); this.guardSourceOptionWrite(o, 'iaid');
		o = s.option(form.Value, 'mac', _('MAC')); this.sourceDhcpv6Options.mac = o; o.modalonly = true; o.depends('type', 'dhcpv6_mac'); this.guardSourceOptionWrite(o, 'mac');
		o = s.option(form.Value, 'lease_file', _('Lease file')); this.sourceDhcpv6Options.leaseFile = o; o.placeholder = '/tmp/odhcpd.leases'; o.modalonly = true; o.depends('type', 'dhcpv6_duid'); o.depends('type', 'dhcpv6_mac'); this.guardSourceOptionWrite(o, 'lease_file');
		o = s.option(form.Value, 'prefix_filter', _('Prefix narrowing'), _('Advanced narrowing after interface prefix matching; it cannot replace the interface.')); this.sourceDhcpv6Options.prefixFilter = o; o.placeholder = '240e:'; o.modalonly = true; o.depends('type', 'dhcpv6_duid'); o.depends('type', 'dhcpv6_mac'); this.guardSourceOptionWrite(o, 'prefix_filter');
		o = s.option(form.Value, 'hostname_hint', _('Hostname hint')); this.sourceDhcpv6Options.hostnameHint = o; o.modalonly = true; o.depends('type', 'dhcpv6_duid'); o.depends('type', 'dhcpv6_mac'); this.guardSourceOptionWrite(o, 'hostname_hint');
		o = s.option(widgets.DeviceSelect, 'interface', _('Interface'), _('Required for DHCPv6 DUID/MAC sources; selected public IPv6 prefixes are the validity source.'));
		this.sourceDhcpv6Options.interface = o;
		o.multiple = true;
		o.cfgvalue = function(sectionId, value) {
			const stored = arguments.length > 1 ? value : (this.data?.[sectionId] || uci.get('qddns', sectionId, 'interface'));
			const normalized = viewRef.interfaceValues(stored);

			if (arguments.length > 1) {
				this.data = this.data || {};
				this.data[sectionId] = normalized;
			}

			return normalized;
		};
		const interfaceWrite = o.write;
		o.write = function(sectionId, value) {
			return interfaceWrite.call(this, sectionId, viewRef.interfaceValues(value));
		};
		o.noaliases = true;
		o.nocreate = true;
		o.depends('type', 'interface');
		o.depends('type', 'dhcpv6_duid');
		o.depends('type', 'dhcpv6_mac');
		this.guardSourceOptionWrite(o, 'interface');
		o = s.option(form.Value, 'probe_url', _('Probe URL')); o.modalonly = true; o.depends('type', 'public_probe'); this.guardSourceOptionWrite(o, 'probe_url');
		o = s.option(form.Value, 'script', _('Script path')); o.modalonly = true; o.depends('type', 'script'); this.guardSourceOptionWrite(o, 'script');

		s = m.section(form.GridSection, 'provider', _('Provider Library'), _('Saved providers become selectable on the rules page after saving and reloading. Names stay editable.'));
		s.addremove = true;
		s.anonymous = false;
		this.useNameCreateFlow(s, {
			placeholder: _('New provider name'),
			description: _('Enter the provider name shown in tables and rule selectors.'),
			emptyMessage: _('Provider name must not be empty.'),
			unnamed: _('Unnamed provider')
		});

		o = s.option(form.Value, 'name', _('Name'), _('Name shown in tables and rule selectors.'));
		o.placeholder = _('Unnamed provider');
		o.modalonly = true;

		o = s.option(form.ListValue, 'type', _('Type'));
		o.value('cloudflare', _('Cloudflare'));
		o.value('dnspod', _('DNSPod'));
		o.value('aliyun', _('Aliyun'));
		o.value('custom_http', _('Custom HTTP'));
		o = s.option(form.Value, 'api_token', _('API token')); o.password = true; o.modalonly = true; o.depends('type', 'cloudflare');
		o = s.option(form.Value, 'secret_id', _('Secret ID')); o.modalonly = true; o.depends('type', 'dnspod');
		o = s.option(form.Value, 'secret_key', _('Secret Key')); o.password = true; o.modalonly = true; o.depends('type', 'dnspod');
		o = s.option(form.Value, 'access_key_id', _('Access Key ID')); o.modalonly = true; o.depends('type', 'aliyun');
		o = s.option(form.Value, 'access_key_secret', _('Access Key Secret')); o.password = true; o.modalonly = true; o.depends('type', 'aliyun');
		o = s.option(form.Value, 'url', _('Custom URL')); o.depends('type', 'custom_http');
		o = s.option(form.Value, 'method', _('HTTP method')); o.placeholder = 'POST'; o.depends('type', 'custom_http');
		o = s.option(form.Value, 'headers_json', _('Headers JSON')); o.modalonly = true; o.depends('type', 'custom_http');
		o = s.option(form.Value, 'body_template', _('Body template')); o.modalonly = true; o.depends('type', 'custom_http');
		o = s.option(form.Value, 'lookup_url', _('Lookup URL')); o.modalonly = true; o.depends('type', 'custom_http');
		o = s.option(form.Value, 'lookup_method', _('Lookup method')); o.modalonly = true; o.depends('type', 'custom_http');
		o = s.option(form.Value, 'lookup_headers_json', _('Lookup headers JSON')); o.modalonly = true; o.depends('type', 'custom_http');
		o = s.option(form.Value, 'lookup_json_pointer', _('Lookup JSON pointer')); o.modalonly = true; o.depends('type', 'custom_http');
		o = s.option(form.Value, 'success_contains', _('Success contains')); o.modalonly = true; o.depends('type', 'custom_http');

		return m.render();
	},

	render: function(data) {
		return this.renderConfigForms(data).then(L.bind(function(formEl) {
			return E('div', { class: 'qddns-settings-page' }, [
				E('div', { class: 'cbi-section qddns-panel qddns-dashboard-note' }, [
					E('h3', {}, _('Settings')),
					E('p', { class: 'cbi-section-descr' }, _('Manage daemon defaults, source definitions, and provider credentials here. Use the source probe panel below to validate saved sources before using them on the rules page.'))
				]),
				this.renderProviderTemplatePanel(),
				E('div', { class: 'qddns-wide-form' }, [formEl]),
				this.renderSourceProbePanel(data.sources)
			]);
		}, this));
	}
});
