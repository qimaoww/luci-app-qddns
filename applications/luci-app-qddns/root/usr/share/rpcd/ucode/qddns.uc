#!/usr/bin/env ucode

'use strict';

import { popen, readfile } from 'fs';
import { cursor } from 'uci';

const uci = cursor();
const qddns_ctl = '/usr/bin/qddnsctl';
const dhcpv6_lease_file = '/tmp/odhcpd.leases';
const dhcpv6_lease_max_bytes = 262144;
const dhcpv6_lease_max_entries = 64;
const dhcpv6_lease_max_prefixes = 8;

function is_valid_log_scope(scope) {
	if (!scope)
		return false;

	return match(scope, /^[A-Za-z0-9_.-]+$/) != null;
}

function has_rule(rule_id) {
	let found = false;

	uci.foreach('qddns', 'rule', function(s) {
		if (s['.name'] == rule_id)
			found = true;
	});

	return found;
}

function safe_unload() {
	if (!uci?.unload)
		return;

	try {
		uci.unload('qddns');
	}
	catch (err) {
		return;
	}
}

function exec_json(command) {
	let p = popen(`${qddns_ctl} ${command} 2>/dev/null`, 'r');
	if (!p)
		return null;

	let data = p.read('all');
	p.close();
	return data ? json(trim(data)) : null;
}

function shell_quote(s) {
	return "'" + replace(s, /'/g, "'\\''") + "'";
}

function section_to_obj(section) {
	return {
		id: section['.name'],
		type: section.type,
		name: section.name || null,
		family: section.family,
		hint: section.hostname_hint || section.address || section.interface || section.duid || section.iaid || null
	};
}

function provider_to_obj(section) {
	return {
		id: section['.name'],
		type: section.type,
		name: section.name || null
	};
}

function rule_to_obj(section) {
	return {
		id: section['.name'],
		enabled: section.enabled == '1',
		name: section.name || null,
		provider: section.provider,
		source: section.source,
		record_type: section.record_type,
		zone: section.zone,
		record_name: section.record_name
	};
}

function rule_status_to_obj(status) {
	if (!status)
		return { ok: false };

	return {
		running: status.running,
		status: status.status,
		current_ip: status.current_ip,
		remote_ip: status.remote_ip,
		last_result: status.last_result,
		last_error: status.last_error,
		last_check: status.last_check,
		next_run: status.next_run
	};
}

function dhcpv6_prefix_filter(prefixes) {
	if (!prefixes || !length(prefixes))
		return null;

	let address = split(prefixes[0], '/')[0] || '';
	let first = split(address, ':')[0] || '';

	return first ? `${first}:` : null;
}

function list_dhcpv6_leases() {
	let content = '';
	let leases = [];

	try {
		content = readfile(dhcpv6_lease_file) || '';
	}
	catch (err) {
		return { ok: true, leases: [], message: 'DHCPv6 lease file is not available' };
	}

	if (length(content) > dhcpv6_lease_max_bytes)
		content = substr(content, 0, dhcpv6_lease_max_bytes);

	for (let line in split(content, '\n')) {
		if (length(leases) >= dhcpv6_lease_max_entries)
			break;

		line = trim(line || '');
		if (!line || substr(line, 0, 1) != '#')
			continue;

		let fields = split(line, /\s+/);
		if (length(fields) < 9)
			continue;

		let prefixes = [];
		for (let field_index = 8; field_index < length(fields); field_index++) {
			if (length(prefixes) >= dhcpv6_lease_max_prefixes)
				break;

			let raw = split(fields[field_index], '/')[0] || '';
			if ((substr(raw, 0, 1) == '2' || substr(raw, 0, 1) == '3') && match(raw, /:/) != null)
				push(prefixes, fields[field_index]);
		}

		if (!length(prefixes))
			continue;

		push(leases, {
			interface: fields[1] || null,
			duid: fields[2] || null,
			iaid: fields[3] || null,
			hostname: fields[4] || null,
			prefixes: prefixes,
			prefix_filter: dhcpv6_prefix_filter(prefixes),
			lease_file: dhcpv6_lease_file
		});
	}

	return { ok: true, leases: leases };
}

const methods = {
	get_overview: {
		call: function() {
			let status = exec_json("--config /etc/config/qddns status") || { ok: false };
			let res = {
				main: {
					enabled: (uci.get('qddns', 'main', 'enabled') || '1') == '1',
					log_level: uci.get('qddns', 'main', 'log_level') || 'info'
				},
				status: status
			};

			safe_unload();
			return res;
		}
	},

	list_sources: {
		call: function() {
			let sources = [];
			uci.foreach('qddns', 'source', function(s) {
				push(sources, section_to_obj(s));
			});
			safe_unload();
			return {
				result: sources
			};
		}
	},

	probe_source: {
		args: { id: 'id' },
		call: function(req) {
			let data = exec_json(`--config /etc/config/qddns sources probe ${shell_quote(req.args.id)}`);
			return data || { ok: false, error: 'probe failed' };
		}
	},

	list_dhcpv6_leases: {
		call: function() {
			return list_dhcpv6_leases();
		}
	},

	list_rules: {
		call: function() {
			let providers = [];
			let rules = [];

			uci.foreach('qddns', 'provider', function(s) { push(providers, provider_to_obj(s)); });
			uci.foreach('qddns', 'rule', function(s) { push(rules, rule_to_obj(s)); });
			safe_unload();

			return {
				providers: providers,
				rules: rules
			};
		}
	},

	run_rule: {
		args: { id: 'id' },
		call: function(req) {
			return exec_json(`--config /etc/config/qddns rules run ${shell_quote(req.args.id)}`) || { ok: false };
		}
	},

	test_rule: {
		args: { id: 'id' },
		call: function(req) {
			return exec_json(`--config /etc/config/qddns rules test ${shell_quote(req.args.id)}`) || { ok: false };
		}
	},

	get_logs: {
		args: { scope: 'scope' },
		call: function(req) {
			let scope = req.args.scope || 'system';

			if (!is_valid_log_scope(scope))
				return { ok: false, scope: scope, content: '', entries: [], error: 'invalid log scope' };

			if (scope != 'system' && !has_rule(scope)) {
				safe_unload();
				return { ok: false, scope: scope, content: '', entries: [], error: 'missing log scope' };
			}

			let content = exec_json(`--config /etc/config/qddns logs ${shell_quote(scope)}`);
			safe_unload();
			return content || { ok: false, scope: scope, content: '', entries: [] };
		}
	},

	get_rule_status: {
		args: { id: 'id' },
		call: function(req) {
			let data = exec_json(`--config /etc/config/qddns rules status ${shell_quote(req.args.id)}`);
			return rule_status_to_obj(data);
		}
	}
};

return { qddns: methods };
