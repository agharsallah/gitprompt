// The main numbers of every run (a report's `trend`, spec.mjs metric), kept across builds in
// target/quality/history.json and handed to the page as history.js, which draws them over time on
// the Trends page and as a sparkline on each card. A report kept as up to date adds nothing: its
// numbers did not change.
import { readFileSync, writeFileSync } from 'node:fs';

/** Runs kept per report: years of daily builds, a few hundred KB at most. */
export const MAX_RUNS = 300;

/** history.json as last written, or empty. */
export function loadHistory(path) {
	try {
		const history = JSON.parse(readFileSync(path, 'utf8'));
		return { reports: history.reports || {} };
	} catch {
		return { reports: {} };
	}
}

/**
 * Adds one run of a report: { at, commit?, values: { key: number } }. The metrics' labels, units
 * and directions are the latest run's. Returns false when the run has no number to keep.
 */
export function addRun(history, id, { at, commit, trend }) {
	const metrics = (trend || []).filter((m) => m && typeof m.key === 'string' && Number.isFinite(m.value));
	if (metrics.length === 0) return false;
	const entry = history.reports[id] || { metrics: [], runs: [] };
	const values = Object.fromEntries(metrics.map((m) => [m.key, m.value]));
	entry.runs = [...entry.runs.filter((run) => run.at !== at), { at, ...(commit ? { commit } : {}), values }].sort((a, b) => a.at - b.at).slice(-MAX_RUNS);
	// A metric the latest run lacks keeps its last definition while older runs still have it.
	const kept = entry.metrics.filter((old) => !metrics.some((m) => m.key === old.key) && entry.runs.some((run) => old.key in run.values));
	entry.metrics = [...metrics.map(({ key, label, unit, better }) => ({ key, label, unit, better })), ...kept];
	history.reports[id] = entry;
	return true;
}

/** Writes history.json and history.js (the script the page loads); sync, like status.js. */
export function writeHistory(history, { json, script }) {
	writeFileSync(json, JSON.stringify(history));
	writeFileSync(script, `qualityHistory(${JSON.stringify(history)});\n`);
}
