// The Time dependencies page, shared by every area: what tools/timing finds under the area's folder,
// which matches block (sleeps, raw timeouts and timing assertions in tests, clock reads in code: a
// new one fails `just timing`, the gate and CI unless the ledger accepts it) and which are only
// reported (timers, stopwatches, polling: design choices to review).
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';

import { LEDGER, compare } from '../../timing/ledger.mjs';
import { RULES, blocksIn } from '../../timing/rules.mjs';
import { scan } from '../../timing/scan.mjs';
import { count, kpis, note, plural, table, toneZero } from './spec.mjs';

/** The inputs of a timing report besides its area's: the rules, the scanner and the ledger. */
export const TIMING_INPUTS = ['tools/timing'];

/** The page for the files under `prefix` (`backend/`, `web/`). */
export async function timingPage(ctx, prefix) {
	ctx.step('scanning for time dependencies (tools/timing)');
	const ledger = JSON.parse(await readFile(join(ctx.repo, LEDGER), 'utf8'));
	const findings = (await scan(ctx.repo)).filter((f) => f.file.startsWith(prefix));
	const { added } = compare(findings, ledger);
	const fresh = new Set(added);
	const blocking = findings.filter((f) => f.policy === 'block').length;
	return {
		headline: { value: count(findings.length), label: 'time dependencies', tone: added.length ? 'bad' : blocking ? 'warn' : 'good' },
		summary: `${plural(findings.length, 'time dependency', 'time dependencies')}: ${count(blocking)} blocking, ${count(findings.length - blocking)} reported · ${added.length ? `${count(added.length)} new` : 'none new'}`,
		sections: [
			kpis([
				{ label: 'New', value: count(added.length), tone: toneZero(added.length), hint: `blocking lines ${LEDGER} does not accept: \`just timing\`, the gate and CI fail` },
				{ label: 'Blocking, accepted', value: count(blocking - added.length), tone: blocking - added.length ? 'warn' : 'good', hint: 'kinds that cause flaky tests or untestable code, each line accepted in the ledger: remove them over time' },
				{ label: 'Reported', value: count(findings.length - blocking), hint: 'design choices (timers, timeouts, stopwatches, polling): review, never failing' },
				...RULES.map((rule) => ({ label: rule.title, value: count(findings.filter((f) => f.rule === rule.id).length), hint: `${rule.id}; blocks: ${blocksIn(rule)}` }))
			]),
			...(added.length
				? [
						table(
							'New: blocking, not in the ledger',
							[{ key: 'place', label: 'Place', mono: true }, { key: 'rule', label: 'Rule' }, { key: 'text', label: 'Code', mono: true }],
							added.map((f) => ({ place: `${f.file}:${f.line}`, rule: titleOf(f.rule), text: f.text })),
							'Remove each (its rule below says how), or have the user accept the line in the ledger.'
						)
					]
				: []),
			table(
				'Every time dependency',
				[
					{ key: 'place', label: 'Place', mono: true },
					{ key: 'rule', label: 'Rule' },
					{ key: 'scope', label: 'In' },
					{ key: 'policy', label: 'Blocks' },
					{ key: 'text', label: 'Code', mono: true }
				],
				findings.map((f) => ({
					place: `${f.file}:${f.line}`,
					rule: titleOf(f.rule),
					scope: f.scope,
					policy: { text: fresh.has(f) ? 'new' : f.policy === 'block' ? 'accepted' : 'reported', tone: fresh.has(f) ? 'bad' : f.policy === 'block' ? 'warn' : undefined },
					text: f.text
				})),
				'Comments are left out; a default parameter that injects the time (`now = Date.now()`) does not count.'
			),
			table(
				'The rules',
				[{ key: 'title', label: 'Rule' }, { key: 'blocks', label: 'Blocks' }, { key: 'why', label: 'What can go wrong' }, { key: 'fix', label: 'Instead' }],
				RULES.map((r) => ({ title: `${r.title} (${r.id})`, blocks: blocksIn(r), why: r.why, fix: r.fix }))
			),
			note(
				`Found by \`just timing\` (tools/timing): every Rust and dashboard source file git knows, line by line. A blocking match fails the check, the quality gate and CI unless ${LEDGER} accepts that line (by its code, so moving it is fine and swapping it is not); \`node tools/timing/cli.mjs --update\` drops accepted lines that are gone, and only the user adds one. What no pattern sees (races, reused ports) is what \`just flaky\` is for: the suites run again and again under load.`
			)
		]
	};
}

const titleOf = (id) => RULES.find((r) => r.id === id)?.title ?? id;
