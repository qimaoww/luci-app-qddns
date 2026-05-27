#!/usr/bin/env ucode

'use strict';

import { popen, readfile, writefile, unlink } from 'fs';
import { cursor } from 'uci';

const uci = cursor();
const qddns_ctl = '/usr/bin/qddnsctl';
const ip_cmd = '/sbin/ip';
const draft_probe_config = '/tmp/qddns-luci-source-probe.conf';
const draft_probe_source_id = 'wizard_probe';
const dhcpv4_lease_file = '/tmp/dhcp.leases';
const dhcpv6_lease_file = '/tmp/odhcpd.leases';
const dhcpv6_lease_max_bytes = 262144;
const dhcpv6_lease_max_entries = 64;
const dhcpv6_lease_max_prefixes = 8;

function is_valid_log_scope(scope) {
	if (!scope)
		return false;

	return match(scope, /^[A-Za-z0-9_.-]+$/) != null;
}

function is_valid_id(id) {
	return is_valid_log_scope(id);
}

function get_source_type(source_id) {
	let source_type = null;

	uci.foreach('qddns', 'source', function(s) {
		if (s['.name'] == source_id)
			source_type = s.type || null;
	});

	return source_type;
}

function is_probe_allowed_source_type(source_type) {
	return source_type == 'local_addr' ||
		source_type == 'interface' ||
		source_type == 'dhcpv6_duid' ||
		source_type == 'dhcpv6_mac';
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

function exec_json_with_config(config_path, command) {
	let p = popen(`${qddns_ctl} --config ${config_path} ${command} 2>/dev/null`, 'r');
	if (!p)
		return null;

	let data = p.read('all');
	p.close();
	return data ? json(trim(data)) : null;
}

function exec_json(command) {
	return exec_json_with_config('/etc/config/qddns', command);
}

function uci_quote(value) {
	value = trim(`${value || ''}`);

	if (!value)
		return null;

	if (match(value, /['\r\n]/) != null)
		return null;

	return `'${value}'`;
}

function draft_source_option(name, value) {
	let quoted = uci_quote(value);

	if (!quoted)
		return null;

	return `\toption ${name} ${quoted}\n`;
}

function draft_source_config(req) {
	let source_type = req.args.type || '';
	let source_name = req.args.name || 'Wizard source';
	let lines = [
		`\nconfig source '${draft_probe_source_id}'\n`
	];

	if (!is_probe_allowed_source_type(source_type))
		return null;

	for (let value in [
		source_name,
		source_type,
		req.args.family,
		req.args.address,
		req.args.interface,
		req.args.duid,
		req.args.iaid,
		req.args.mac,
		req.args.lease_file,
		req.args.hostname_hint,
		req.args.prefix_filter
	]) {
		if (value && uci_quote(value) == null)
			return null;
	}

	push(lines, draft_source_option('name', source_name));
	push(lines, draft_source_option('type', source_type));
	push(lines, draft_source_option('family', req.args.family));
	push(lines, draft_source_option('address', req.args.address));
	push(lines, draft_source_option('interface', req.args.interface));
	push(lines, draft_source_option('duid', req.args.duid));
	push(lines, draft_source_option('iaid', req.args.iaid));
	push(lines, draft_source_option('mac', req.args.mac));
	push(lines, draft_source_option('lease_file', req.args.lease_file));
	push(lines, draft_source_option('hostname_hint', req.args.hostname_hint));
	push(lines, draft_source_option('prefix_filter', req.args.prefix_filter));

	let config = '';
	for (let line in lines)
		if (line)
			config += line;

	return config;
}

function source_family(section) {
	let family = section.family || '';

	if (family == 'ipv4' || family == 'ipv6')
		return family;

	if (section.type == 'dhcpv6_duid' || section.type == 'dhcpv6_mac')
		return 'ipv6';

	return null;
}

function section_to_obj(section) {
	return {
		id: section['.name'],
		type: section.type,
		name: section.name || null,
		family: source_family(section),
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

function is_public_ipv6(address) {
	let first = substr(address || '', 0, 1);

	return match(address || '', /:/) != null &&
		(first == '2' || first == '3') &&
		substr(address, 0, 9) != '2001:db8:';
}

function is_private_ipv4(address) {
	address = address || '';

	return substr(address, 0, 3) == '10.' ||
		substr(address, 0, 8) == '192.168.' ||
		(substr(address, 0, 4) == '172.' && match(address, /^172[.](1[6-9]|2[0-9]|3[0-1])[.]/) != null);
}

function push_unique(list, value) {
	if (!value)
		return;

	for (let index = 0; index < length(list); index++)
		if (list[index] == value)
			return;

	push(list, value);
}

function normalize_dhcpv6_mac(mac) {
	if (!mac)
		return null;

	mac = lc(mac);
	mac = replace(mac, /[:-]/g, '');

	if (length(mac) != 12 || match(mac, /^[0-9a-f]{12}$/) == null)
		return null;

	return join(':', [
		substr(mac, 0, 2),
		substr(mac, 2, 2),
		substr(mac, 4, 2),
		substr(mac, 6, 2),
		substr(mac, 8, 2),
		substr(mac, 10, 2)
	]);
}

function dhcpv6_duid_mac(duid) {
	if (!duid)
		return null;

	duid = lc(duid);
	if (length(duid) < 12 || match(duid, /^[0-9a-f]+$/) == null)
		return null;

	return normalize_dhcpv6_mac(substr(duid, length(duid) - 12, 12));
}

function strip_lan_suffix(hostname) {
	if (!hostname)
		return null;

	return replace(hostname, /\.lan$/, '');
}

function host_entry(entries, mac) {
	if (!mac)
		return null;

	if (!entries[mac]) {
		entries[mac] = {
			mac: mac,
			hostname: null,
			ipv4: [],
			prefixes: []
		};
	}

	return entries[mac];
}

function add_dhcpv4_lease_entries(entries) {
	let content = '';

	try {
		content = readfile(dhcpv4_lease_file) || '';
	}
	catch (err) {
		content = '';
	}

	if (length(content) > dhcpv6_lease_max_bytes)
		content = substr(content, 0, dhcpv6_lease_max_bytes);

	for (let line in split(content, '\n')) {
		line = trim(line || '');
		if (!line)
			continue;

		let fields = split(line, /\s+/);
		if (length(fields) < 4)
			continue;

		let entry = host_entry(entries, normalize_dhcpv6_mac(fields[1]));
		if (!entry)
			continue;

		if (fields[3] && fields[3] != '*')
			entry.hostname = entry.hostname || strip_lan_suffix(fields[3]);

		if (is_private_ipv4(fields[2]))
			push_unique(entry.ipv4, fields[2]);
	}
}

function add_dhcpv6_lease_entry(entries, fields, prefixes) {
	let mac = dhcpv6_duid_mac(fields[2]);
	let entry = host_entry(entries, mac);
	if (!entry)
		return;

	entry.duid = fields[2] || null;
	entry.iaid = fields[3] || null;
	entry.interface = fields[1] || null;
	entry.hostname = entry.hostname || fields[4] || null;
	entry.lease_file = dhcpv6_lease_file;

	for (let prefix in prefixes)
		push_unique(entry.prefixes, prefix);
}

function add_ndp_entries(entries) {
	let p = popen(`${ip_cmd} -6 neigh show 2>/dev/null`, 'r');
	if (!p)
		return;

	let output = p.read('all') || '';
	p.close();

	for (let line in split(output, '\n')) {
		line = trim(line || '');
		if (!line)
			continue;

		let fields = split(line, /\s+/);
		let address = fields[0] || '';
		let lladdr_index = null;

		for (let index = 0; index < length(fields); index++) {
			if (fields[index] == 'lladdr') {
				lladdr_index = index;
				break;
			}
		}

		if (lladdr_index == null)
			continue;

		let mac = normalize_dhcpv6_mac(fields[lladdr_index + 1]);
		let entry = host_entry(entries, mac);
		if (!entry || !is_public_ipv6(address))
			continue;

		entry.interface = entry.interface || fields[2] || null;
		push_unique(entry.prefixes, `${address}/128`);
	}
}

function add_ipv4_neighbor_entries(entries) {
	let p = popen(`${ip_cmd} -4 neigh show 2>/dev/null`, 'r');
	if (!p)
		return;

	let output = p.read('all') || '';
	p.close();

	for (let line in split(output, '\n')) {
		line = trim(line || '');
		if (!line)
			continue;

		let fields = split(line, /\s+/);
		let address = fields[0] || '';
		let lladdr_index = null;

		if (!is_private_ipv4(address))
			continue;

		for (let index = 0; index < length(fields); index++) {
			if (fields[index] == 'lladdr') {
				lladdr_index = index;
				break;
			}
		}

		if (lladdr_index == null)
			continue;

		let mac = normalize_dhcpv6_mac(fields[lladdr_index + 1]);
		let entry = entries[mac];
		if (!entry)
			continue;

		push_unique(entry.ipv4, address);
	}
}

function list_dhcpv6_leases(mode) {
	let content = '';
	let entries = {};
	mode = mode == 'mac' ? 'mac' : 'duid';

	try {
		content = readfile(dhcpv6_lease_file) || '';
	}
	catch (err) {
		content = '';
	}

	if (length(content) > dhcpv6_lease_max_bytes)
		content = substr(content, 0, dhcpv6_lease_max_bytes);

	for (let line in split(content, '\n')) {
		if (length(keys(entries)) >= dhcpv6_lease_max_entries)
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
			if (is_public_ipv6(raw))
				push(prefixes, fields[field_index]);
		}

		if (!length(prefixes))
			continue;

		add_dhcpv6_lease_entry(entries, fields, prefixes);
	}

	add_dhcpv4_lease_entries(entries);
	add_ndp_entries(entries);
	add_ipv4_neighbor_entries(entries);

	let leases = [];
	for (let mac in entries) {
		let entry = entries[mac];
		if (!length(entry.prefixes))
			continue;

		if (mode == 'mac') {
			delete entry.duid;
			delete entry.iaid;
			delete entry.lease_file;
		}
		push(leases, entry);
	}

	return { ok: true, leases: leases };
}

const methods = {
	get_overview: {
		call: function() {
			let status = exec_json('status') || { ok: false };
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
			let id = req.args.id || '';

			if (!is_valid_id(id))
				return { ok: false, error: 'invalid source id' };

			let source_type = get_source_type(id);
			if (!source_type) {
				safe_unload();
				return { ok: false, error: 'missing source' };
			}

			if (!is_probe_allowed_source_type(source_type)) {
				safe_unload();
				return { ok: false, error: 'probe not allowed for source type' };
			}

			let data = exec_json(`sources probe ${id}`);
			safe_unload();
			return data || { ok: false, error: 'probe failed' };
		}
	},

	probe_source_draft: {
		args: {
			name: 'name',
			type: 'type',
			family: 'family',
			address: 'address',
			interface: 'interface',
			duid: 'duid',
			iaid: 'iaid',
			mac: 'mac',
			lease_file: 'lease_file',
			hostname_hint: 'hostname_hint',
			prefix_filter: 'prefix_filter'
		},
		call: function(req) {
			let source_config = draft_source_config(req);
			if (!source_config)
				return { ok: false, error: 'invalid draft source' };

			let base_config = '';
			try {
				base_config = readfile('/etc/config/qddns') || '';
			}
			catch (err) {
				base_config = '';
			}

			try {
				writefile(draft_probe_config, base_config + source_config);
			}
			catch (err) {
				return { ok: false, error: 'unable to create draft source probe' };
			}

			let data = exec_json_with_config(draft_probe_config, `sources probe ${draft_probe_source_id}`) || { ok: false, error: 'probe failed' };

			try {
				unlink(draft_probe_config);
			}
			catch (err) {
				return data;
			}

			return data;
		}
	},

	list_dhcpv6_leases: {
		args: { mode: 'mode' },
		call: function(req) {
			return list_dhcpv6_leases(req.args.mode || 'duid');
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
			let id = req.args.id || '';

			if (!is_valid_id(id) || !has_rule(id)) {
				safe_unload();
				return { ok: false, error: 'invalid rule id' };
			}

			let data = exec_json(`rules run ${id}`) || { ok: false };
			safe_unload();
			return data;
		}
	},

	test_rule: {
		args: { id: 'id' },
		call: function(req) {
			let id = req.args.id || '';

			if (!is_valid_id(id) || !has_rule(id)) {
				safe_unload();
				return { ok: false, error: 'invalid rule id' };
			}

			let data = exec_json(`rules test ${id}`) || { ok: false };
			safe_unload();
			return data;
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

			let content = exec_json(`logs ${scope}`);
			safe_unload();
			return content || { ok: false, scope: scope, content: '', entries: [] };
		}
	},

	get_rule_status: {
		args: { id: 'id' },
		call: function(req) {
			let id = req.args.id || '';

			if (!is_valid_id(id) || !has_rule(id)) {
				safe_unload();
				return { ok: false, error: 'invalid rule id' };
			}

			let data = exec_json(`rules status ${id}`);
			safe_unload();
			return rule_status_to_obj(data);
		}
	}
};

return { qddns: methods };
