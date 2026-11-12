// The Rust dependencies: security advisories, licenses, banned or duplicate crates and sources
// (cargo-deny, backend/deny.toml), and dependencies declared but unused (cargo-machete). The same
// checks as `just deps`.
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';

import { kpis, note, plural, table, toneZero } from '../../lib/spec.mjs';

const CHECKS = ['advisories', 'bans', 'licenses', 'sources'];

export default {
	title: 'Dependencies',
	description: 'Advisories, licenses, duplicates and sources (cargo-deny), and unused crates (cargo-machete).',
	needs: [
		{ command: 'cargo-deny', install: 'cargo install --locked cargo-deny' },
		{ command: 'cargo-machete', install: 'cargo install --locked cargo-machete' }
	],
	async run(ctx) {
		const deny = await ctx.exec('cargo', ['deny', '--locked', '--format', 'json', 'check'], { cwd: ctx.backend, label: 'cargo deny check', allowFailure: true });
		const { diagnostics, summary } = parseDeny(deny.stderr);
		if (!summary) throw new Error(`cargo deny gave no summary (exit ${deny.code}); see the log`);
		const machete = await ctx.exec('cargo', ['machete'], { cwd: ctx.backend, label: 'cargo machete', allowFailure: true });
		const unused = parseMachete(machete.stdout);
		const lock = await readFile(join(ctx.backend, 'Cargo.lock'), 'utf8');
		return page(diagnostics, summary, unused, (lock.match(/^\[\[package\]\]$/gm) || []).length);
	}
};

/** cargo deny's JSON lines: the error and warning diagnostics, and the per-check summary. */
export function parseDeny(stderr) {
	const diagnostics = [];
	let summary = null;
	for (const line of stderr.split('\n')) {
		if (!line.startsWith('{')) continue;
		const entry = JSON.parse(line);
		if (entry.type === 'summary') summary = entry.fields;
		if (entry.type === 'diagnostic' && ['error', 'warning'].includes(entry.fields.severity)) {
			diagnostics.push({ severity: entry.fields.severity, code: entry.fields.code || '', message: entry.fields.message });
		}
	}
	return { diagnostics, summary };
}

/** `crate -- ./path/Cargo.toml:` then one tab-indented line per unused dependency. */
export function parseMachete(stdout) {
	const unused = [];
	let crate = null;
	for (const line of stdout.split('\n')) {
		const header = line.match(/^(\S+) -- (.+):$/);
		if (header) crate = header[1];
		else if (crate && /^\t\S/.test(line)) unused.push({ crate, dependency: line.trim() });
	}
	return unused;
}

function page(diagnostics, summary, unused, lockedCrates) {
	const of = (check, key) => summary[check]?.[key] ?? 0;
	const errors = CHECKS.reduce((t, c) => t + of(c, 'errors'), 0);
	const warnings = CHECKS.reduce((t, c) => t + of(c, 'warnings'), 0);
	const findings = errors + warnings + unused.length;
	const sections = [
		kpis([
			{ label: 'Advisories', value: of('advisories', 'errors') + of('advisories', 'warnings'), tone: toneZero(of('advisories', 'errors')) },
			{ label: 'License problems', value: of('licenses', 'errors'), tone: toneZero(of('licenses', 'errors')) },
			{ label: 'Bans and duplicates', value: of('bans', 'errors') + of('bans', 'warnings'), tone: of('bans', 'errors') ? 'bad' : of('bans', 'warnings') ? 'warn' : 'good' },
			{ label: 'Unused dependencies', value: unused.length, tone: toneZero(unused.length) },
			{ label: 'Crates in Cargo.lock', value: lockedCrates }
		]),
		table(
			'cargo deny, per check',
			[
				{ key: 'check', label: 'Check', mono: true },
				{ key: 'errors', label: 'Errors', align: 'right' },
				{ key: 'warnings', label: 'Warnings', align: 'right' },
				{ key: 'notes', label: 'Notes', align: 'right' }
			],
			CHECKS.map((check) => ({
				check,
				errors: { text: of(check, 'errors'), tone: of(check, 'errors') ? 'bad' : 'good', sort: of(check, 'errors') },
				warnings: { text: of(check, 'warnings'), tone: of(check, 'warnings') ? 'warn' : undefined, sort: of(check, 'warnings') },
				notes: of(check, 'notes')
			})),
			'Rules: backend/deny.toml. CI runs the same check (`just deps`) and fails on errors.'
		)
	];
	if (findings === 0) sections.push(note('Clean: no advisory, license, ban or source problem, and every declared dependency is used.', 'good'));
	if (diagnostics.length) {
		sections.push(
			table(
				'cargo deny findings',
				[
					{ key: 'severity', label: 'Severity' },
					{ key: 'code', label: 'Code', mono: true },
					{ key: 'message', label: 'Message' }
				],
				diagnostics.map((d) => ({ severity: { text: d.severity, tone: d.severity === 'error' ? 'bad' : 'warn' }, code: d.code, message: d.message }))
			)
		);
	}
	if (unused.length) {
		sections.push(table('Unused dependencies (cargo machete): remove them from Cargo.toml', [{ key: 'crate', label: 'Crate', mono: true }, { key: 'dependency', label: 'Dependency', mono: true }], unused));
	}
	return {
		headline: { value: findings, label: findings === 1 ? 'finding' : 'findings', tone: errors || unused.length ? 'bad' : warnings ? 'warn' : 'good' },
		summary: `${plural(errors, 'error')}, ${plural(warnings, 'warning')} from cargo deny · ${plural(unused.length, 'unused dependency', 'unused dependencies')} · ${lockedCrates} crates locked`,
		sections
	};
}
