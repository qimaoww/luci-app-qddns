#!/usr/bin/env ucode

'use strict';

import { open, popen, stat, writefile, unlink } from 'fs';
import { cursor } from 'uci';

const uci = cursor();
const qddns_ctl = '/usr/bin/qddnsctl';
const ip_cmd = '/sbin/ip';
const mktemp_cmd = '/bin/mktemp';
const draft_probe_config_prefix = '/tmp/qddns-luci-source-probe.';
const draft_probe_config_template = `${draft_probe_config_prefix}XXXXXX`;
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

function create_draft_probe_config_path() {
	let p = popen(`${mktemp_cmd} ${draft_probe_config_template} 2>/dev/null`, 'r');
	if (!p)
		return null;

	let path = trim(p.read('all') || '');
	p.close();
	if (!path || substr(path, 0, length(draft_probe_config_prefix)) != draft_probe_config_prefix)
		return null;

	return path;
}

function exec_json_with_config(config_path, command) {
	let p = popen(`${qddns_ctl} --config ${config_path} ${command} 2>&1`, 'r');
	if (!p)
		return null;

	let data = p.read('all');
	p.close();
	let output = trim(data || '');

	if (output) {
		let parsed = null;
		try {
			parsed = json(output);
		}
		catch (err) {
			parsed = null;
		}

		if (parsed)
			return parsed;

		return { ok: false, error: output || 'command failed' };
	}

	return null;
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

function push_unique(list, value) {
	if (!value)
		return;

	for (let index = 0; index < length(list); index++)
		if (list[index] == value)
			return;

	push(list, value);
}

function interface_values(value) {
	let values = [];

	if (type(value) == 'array') {
		for (let item in value) {
			for (let part in split(`${item || ''}`, /,+/)) {
				part = trim(part || '');
				if (part)
					push_unique(values, part);
			}
		}
	} else {
		for (let part in split(`${value || ''}`, /,+/)) {
			part = trim(part || '');
			if (part)
				push_unique(values, part);
		}
	}

	return values;
}

function interface_value(value) {
	return join(',', interface_values(value));
}

function draft_lease_file(value) {
	value = trim(`${value || ''}`);
	if (!value)
		return '';

	return value == dhcpv6_lease_file ? value : null;
}

function draft_source_config(req) {
	let source_type = req.args.type || '';
	let source_name = req.args.name || 'Wizard source';
	let lease_file = draft_lease_file(req.args.lease_file);
	let lines = [
		`\nconfig source '${draft_probe_source_id}'\n`
	];

	if (!is_probe_allowed_source_type(source_type))
		return null;
	if (lease_file == null)
		return null;

	for (let value in [
		source_name,
		source_type,
		req.args.family,
		req.args.address,
		interface_value(req.args.interface),
		req.args.duid,
		req.args.iaid,
		req.args.mac,
		lease_file,
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
	push(lines, draft_source_option('interface', interface_value(req.args.interface)));
	push(lines, draft_source_option('duid', req.args.duid));
	push(lines, draft_source_option('iaid', req.args.iaid));
	push(lines, draft_source_option('mac', req.args.mac));
	push(lines, draft_source_option('lease_file', lease_file));
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

	if (section.type == 'dhcpv6_duid' || section.type == 'dhcpv6_mac')
		return 'ipv6';

	if (family == 'ipv4' || family == 'ipv6')
		return family;

	return null;
}

function section_to_obj(section) {
	let interfaces = interface_value(section.interface);

	return {
		id: section['.name'],
		type: section.type,
		name: section.name || null,
		family: source_family(section),
		hint: section.hostname_hint || section.address || interfaces || section.duid || section.iaid || null,
		interface: interfaces || null
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
			interfaces: [],
			prefixes: []
		};
	}

	return entries[mac];
}

function read_limited_regular_file(path, max_bytes) {
	let info = null;
	let file = null;
	let content = '';

	try {
		info = stat(path);
	}
	catch (err) {
		return '';
	}

	if (!info || info.type != 'file')
		return '';

	try {
		file = open(path, 'r');
		if (!file)
			return '';

		content = file.read(max_bytes + 1) || '';
	}
	catch (err) {
		content = '';
	}

	if (file) {
		try {
			file.close();
		}
		catch (err) {
			return content || '';
		}
	}

	if (length(content) > max_bytes)
		return substr(content, 0, max_bytes);

	return content;
}

function add_dhcpv4_lease_entries(entries) {
	let content = read_limited_regular_file(dhcpv4_lease_file, dhcpv6_lease_max_bytes);

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
	push_unique(entry.interfaces, fields[1]);
	entry.hostname = entry.hostname || fields[4] || null;
	entry.lease_file = dhcpv6_lease_file;

	for (let prefix in prefixes)
		push_unique(entry.prefixes, prefix);
}

function refresh_ndp_for_interface(lan_iface) {
	if (!lan_iface)
		return;

	popen(`ping6 -c 1 -W 1 -I ${lan_iface} ff02::1 >/dev/null 2>&1`, 'r')?.close();
}

function mac_to_eui64_suffix(mac) {
	if (!mac)
		return null;

	let parts = split(mac, ':');
	if (length(parts) != 6)
		return null;

	let b0 = hex(parts[0]) ^ 0x02;
	return sprintf('%02x%s:%sff:fe%s:%s%s',
		b0, parts[1], parts[2], parts[3], parts[4], parts[5]);
}

function get_lan_prefixes(lan_iface) {
	if (!lan_iface)
		return [];

	let p = popen(`${ip_cmd} -6 route show dev ${lan_iface} proto static 2>/dev/null`, 'r');
	if (!p)
		return [];

	let output = p.read('all') || '';
	p.close();

	let prefixes = [];
	for (let line in split(output, '\n')) {
		line = trim(line || '');
		if (!line)
			continue;

		let parts = split(line, /\s+/);
		let prefix = parts[0] || '';
		if (!match(prefix, /^[23][0-9a-f]+:.*\/64$/))
			continue;

		let network = split(prefix, '/')[0] || '';
		if (network && is_public_ipv6(network))
			push_unique(prefixes, replace(network, /::?$/, ''));
	}

	return prefixes;
}

function probe_slaac_addresses(entries, lan_iface) {
	if (!lan_iface)
		return;

	let prefixes = get_lan_prefixes(lan_iface);
	if (!length(prefixes))
		return;

	for (let mac in entries) {
		let entry = entries[mac];
		let suffix = mac_to_eui64_suffix(mac);
		if (!suffix)
			continue;

		for (let prefix in prefixes) {
			let addr = prefix + ':' + suffix;
			push_unique(entry.prefixes, `${addr}/128`);
		}
	}
}

function add_ndp_entries(entries) {
	let lan_iface = uci.get('qddns', 'main', 'lan_interface') || '';
	if (lan_iface)
		refresh_ndp_for_interface(lan_iface);

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

		push_unique(entry.interfaces, fields[2]);
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

function list_interfaces() {
	let p = popen(`${ip_cmd} -o link show 2>/dev/null`, 'r');
	let interfaces = [];

	if (!p)
		return { ok: true, interfaces: interfaces };

	let output = p.read('all') || '';
	p.close();

	for (let line in split(output, '\n')) {
		line = trim(line || '');
		if (!line)
			continue;

		let fields = split(line, ':');
		let name = length(fields) > 1 ? trim(fields[1] || '') : null;
		name = name ? (split(name, '@')[0] || name) : null;
		if (!name || name == 'lo')
			continue;

		push_unique(interfaces, name);
	}

	return { ok: true, interfaces: interfaces };
}

function list_dhcpv6_leases(mode) {
	let content = read_limited_regular_file(dhcpv6_lease_file, dhcpv6_lease_max_bytes);
	let entries = {};
	mode = mode == 'mac' ? 'mac' : 'duid';

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
	probe_slaac_addresses(entries, uci.get('qddns', 'main', 'lan_interface') || '');
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
		entry.host_interface = join(',', entry.interfaces);
		delete entry.interfaces;
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

	list_interfaces: {
		call: function() {
			return list_interfaces();
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

			let draft_probe_config = create_draft_probe_config_path();
			if (!draft_probe_config)
				return { ok: false, error: 'unable to create draft source probe' };

			try {
				writefile(draft_probe_config, source_config);
			}
			catch (err) {
				try {
					unlink(draft_probe_config);
				}
				catch (cleanup_err) {
					return { ok: false, error: 'unable to create draft source probe' };
				}
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
