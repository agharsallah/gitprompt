// The Complexity page, shared by every area: each report measures its code its own way (clippy,
// ESLint) and hands over the same shape; this draws it.
//
//   functions:  [{ name, place, unit, complexity, lines }]   every function measured
//   files:      [{ file, unit, lines, limit }]                every source file
//   limits:     { complexity, lines }                         a function's limits (the gate's)
//   findings:   [{ place, rule, message }]                    other limits the gate fails on (nesting, parameters…)
//   exemptions: [{ place, rule, reason }]                     functions kept complex on purpose
//   measure:    one line on what "complexity" counts here
import { bars, count, groupBy, kpis, metric, note, plural, table, toneBelow, toneZero } from './spec.mjs';

const LISTED = 25;
/** Thirds of the limit, then over it: 15 → ≤5, 6–10, 11–15, 16+. */
function bands(limit) {
	const third = Math.round(limit / 3);
	return [
		{ label: `simple (≤${third})`, max: third, tone: 'good' },
		{ label: `moderate (${third + 1}–${2 * third})`, max: 2 * third, tone: 'good' },
		{ label: `complex (${2 * third + 1}–${limit})`, max: limit, tone: 'warn' },
		{ label: `over the limit (${limit + 1}+)`, max: Infinity, tone: 'bad' }
	];
}

/** The value at share q (0–1) of the sorted values. */
export function quantile(values, q) {
	if (!values.length) return 0;
	const sorted = [...values].sort((a, b) => a - b);
	return sorted[Math.min(sorted.length - 1, Math.floor(q * sorted.length))];
}

export function complexityPage({ functions, files, limits, findings = [], exemptions = [], measure }) {
	const scores = functions.map((f) => f.complexity);
	const tooComplex = functions.filter((f) => f.complexity > limits.complexity);
	const tooLong = functions.filter((f) => f.lines > limits.lines);
	const bigFiles = files.filter((f) => f.lines > f.limit);
	const failing = tooComplex.length + tooLong.length + bigFiles.length + findings.length;
	const p90 = quantile(scores, 0.9);
	const sections = [
		kpis([
			{ label: 'Over a limit', value: failing, tone: toneZero(failing), hint: 'functions, files and blocks the gate fails on' },
			{ label: 'Functions', value: count(functions.length) },
			{ label: 'Median complexity', value: quantile(scores, 0.5) },
			{ label: '90th percentile', value: p90, tone: toneBelow(p90, limits.complexity / 2, limits.complexity) },
			{ label: 'Most complex', value: Math.max(0, ...scores), tone: toneBelow(Math.max(0, ...scores), limits.complexity, limits.complexity * 1.5) },
			{ label: 'Kept on purpose', value: exemptions.length, tone: exemptions.length ? 'warn' : 'good', hint: 'exemptions with a reason' }
		]),
		note(
			failing
				? 'The gate fails on these. Split the function by step or concept, flatten with early returns, name the parameter group as a type; keep it only when the logic is inherently that complex, with the reason written down.'
				: `Every function within the limits (complexity ≤ ${limits.complexity}, ≤ ${limits.lines} lines) and every file within its size.`,
			failing ? 'bad' : 'good'
		),
		note(measure),
		bars('Functions by complexity', distribution(scores, limits.complexity),`Limit ${limits.complexity}. A healthy codebase is mostly simple, with a few complex functions that earn it.`),
		functionTable('Most complex functions', functions, (f) => f.complexity, limits),
		functionTable('Longest functions', functions, (f) => f.lines, limits),
		fileTable(files),
		unitTable(functions, limits)
	];
	if (findings.length) sections.push(findingTable(findings));
	sections.push(exemptionTable(exemptions));
	return {
		headline: { value: failing, label: 'over a limit', tone: toneZero(failing) },
		trend: [metric('over_limit', 'Over a limit', failing, 'count', 'lower'), metric('p90', '90th percentile complexity', p90, 'score', 'lower'), metric('max', 'Most complex function', Math.max(0, ...scores), 'score', 'lower'), metric('exemptions', 'Kept on purpose', exemptions.length, 'count', 'lower')],
		summary: `${plural(functions.length, 'function')}, 90th percentile complexity ${p90}, ${plural(failing, 'finding')}, ${plural(exemptions.length, 'exemption')}`,
		sections
	};
}

function distribution(scores, limit) {
	let floor = -Infinity;
	return bands(limit).map((band) => {
		const n = scores.filter((s) => s > floor && s <= band.max).length;
		floor = band.max;
		return { label: band.label, value: scores.length ? n / scores.length : 0, text: count(n), tone: band.tone };
	});
}

const toneCount = (n) => ({ text: count(n), sort: n, tone: toneZero(n) });
const over = (value, limit) => ({ text: count(value), sort: value, tone: value > limit ? 'bad' : value > limit * 0.75 ? 'warn' : undefined });

function functionTable(title, functions, by, limits) {
	const top = [...functions].sort((a, b) => by(b) - by(a)).slice(0, LISTED);
	return table(
		title,
		[
			{ key: 'name', label: 'Function', mono: true },
			{ key: 'complexity', label: 'Complexity', align: 'right' },
			{ key: 'lines', label: 'Lines', align: 'right' },
			{ key: 'place', label: 'Where', mono: true }
		],
		top.map((f) => ({ name: f.name, complexity: over(f.complexity, limits.complexity), lines: over(f.lines, limits.lines), place: f.place }))
	);
}

function fileTable(files) {
	const top = [...files].sort((a, b) => b.lines / b.limit - a.lines / a.limit).slice(0, LISTED);
	return table(
		'Largest files (against their limit)',
		[
			{ key: 'file', label: 'File', mono: true },
			{ key: 'lines', label: 'Lines', align: 'right' },
			{ key: 'limit', label: 'Limit', align: 'right' }
		],
		top.map((f) => ({ file: f.file, lines: over(f.lines, f.limit), limit: f.limit })),
		`${plural(files.length, 'file')} measured. A file over its limit holds several ideas: split it by concept.`
	);
}

function unitTable(functions, limits) {
	const rows = groupBy(functions, (f) => f.unit).map(([unit, list]) => {
		const scores = list.map((f) => f.complexity);
		const mean = scores.reduce((a, b) => a + b, 0) / scores.length;
		const worst = Math.max(...scores);
		return {
			unit,
			functions: list.length,
			mean: { text: mean.toFixed(1), sort: mean },
			p90: quantile(scores, 0.9),
			max: over(worst, limits.complexity),
			overLimit: toneCount(list.filter((f) => f.complexity > limits.complexity || f.lines > limits.lines).length)
		};
	});
	return table(
		'By part',
		[
			{ key: 'unit', label: 'Part', mono: true },
			{ key: 'functions', label: 'Functions', align: 'right' },
			{ key: 'mean', label: 'Mean', align: 'right' },
			{ key: 'p90', label: '90th pct', align: 'right' },
			{ key: 'max', label: 'Max', align: 'right' },
			{ key: 'overLimit', label: 'Over a limit', align: 'right' }
		],
		rows
	);
}

function findingTable(findings) {
	return table(
		'Other limits the gate fails on',
		[
			{ key: 'place', label: 'Where', mono: true },
			{ key: 'rule', label: 'Rule', mono: true },
			{ key: 'message', label: 'Message' }
		],
		findings.map((f) => ({ place: f.place, rule: { text: f.rule, tone: 'bad' }, message: f.message }))
	);
}

function exemptionTable(exemptions) {
	if (!exemptions.length) return note('No function is exempt from a complexity limit.', 'good');
	return table(
		'Kept complex on purpose',
		[
			{ key: 'place', label: 'Where', mono: true },
			{ key: 'rule', label: 'Limit', mono: true },
			{ key: 'reason', label: 'Reason' }
		],
		exemptions,
		'Each one is a decision the user approved (the guard asks): review that the reason still holds.'
	);
}
