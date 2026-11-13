#!/usr/bin/env node
// Time dependencies in the backend and the dashboard (rules.mjs): `just timing`.
//
//   node tools/timing/cli.mjs            check: fails (exit 1) on a new blocking one the ledger lacks
//   node tools/timing/cli.mjs --list     every one, blocking and reported, by rule, with file and line
//   node tools/timing/cli.mjs --json     the same as JSON: { rules, findings, added, gone }
//   node tools/timing/cli.mjs --update   drop the ledger's lines that are gone (never adds one)
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { LEDGER, compare, shrink } from './ledger.mjs';
import { RULES, blocksIn } from './rules.mjs';
import { scan } from './scan.mjs';

const REPO = join(dirname(fileURLToPath(import.meta.url)), '..', '..');

const readLedger = () => JSON.parse(readFileSync(join(REPO, LEDGER), 'utf8'));
const ruleOf = (id) => RULES.find((r) => r.id === id);

function list(findings) {
	for (const rule of RULES) {
		const mine = findings.filter((f) => f.rule === rule.id);
		if (!mine.length) continue;
		console.log(`\n${rule.title} (${rule.id}; blocks: ${blocksIn(rule)}): ${mine.length}`);
		for (const f of mine) console.log(`  ${f.policy === 'block' ? '■' : '·'} ${f.file}:${f.line}  ${f.text}`);
	}
	const blocking = findings.filter((f) => f.policy === 'block').length;
	console.log(`\n${findings.length} time dependencies: ${blocking} blocking (■, each accepted in ${LEDGER}), ${findings.length - blocking} reported (·).`);
	console.log('Why each rule matters and what to do instead: tools/timing/rules.mjs.');
}

function check(findings, { added, gone }) {
	if (added.length) {
		console.error(`New time dependencies that block: remove them (each rule says how), or ask the user to accept the line in ${LEDGER}.\n`);
		for (const rule of RULES) {
			const mine = added.filter((f) => f.rule === rule.id);
			if (!mine.length) continue;
			console.error(`${rule.title} (${rule.id})`);
			for (const f of mine) console.error(`  ${f.file}:${f.line}  ${f.text}`);
			console.error(`  why: ${rule.why}\n  instead: ${rule.fix}\n`);
		}
		return 1;
	}
	const blocking = findings.filter((f) => f.policy === 'block').length;
	const hint = gone.length ? ` ${gone.length} accepted line${gone.length === 1 ? ' is' : 's are'} gone: run \`node tools/timing/cli.mjs --update\` to drop them.` : '';
	console.log(`${findings.length} time dependencies (${blocking} blocking, all accepted; ${findings.length - blocking} reported), none new.${hint}`);
	return 0;
}

const args = new Set(process.argv.slice(2));
const findings = await scan(REPO);
if (args.has('--list')) {
	list(findings);
} else if (args.has('--json')) {
	const rules = RULES.map((r) => ({ id: r.id, title: r.title, scope: r.scope, blocks: blocksIn(r), why: r.why, fix: r.fix }));
	console.log(JSON.stringify({ rules, findings, ...compare(findings, readLedger()) }, null, 2));
} else if (args.has('--update')) {
	const ledger = readLedger();
	const { added } = compare(findings, ledger);
	if (added.length) {
		console.error(`Not updated: ${added.length} new blocking line(s), first ${added[0].file}:${added[0].line} (${ruleOf(added[0].rule).title}). --update only drops lines.`);
		process.exitCode = 1;
	} else {
		writeFileSync(join(REPO, LEDGER), `${JSON.stringify(shrink(ledger, findings), null, '\t')}\n`);
		console.log(`${LEDGER}: only the accepted lines still found are kept.`);
	}
} else {
	process.exitCode = check(findings, compare(findings, readLedger()));
}
