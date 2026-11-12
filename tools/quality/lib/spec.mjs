// The building blocks of a report page. A report's run() returns
//   { headline: { value, label, tone }, summary, sections: [ ...blocks ] }
// and the dashboard (hub/render.js) draws every block the same way. Tones: good, warn, bad, neutral.

/** A row of headline numbers. items: [{ label, value, tone?, hint? }] */
export const kpis = (items) => ({ type: 'kpis', items });

/** Horizontal bars, each a share from 0 to 1. items: [{ label, value, text?, tone? }] */
export const bars = (title, items, note) => ({ type: 'bars', title, items, note });

/**
 * A sortable table, filterable when long. columns: [{ key, label, align?: 'right', mono? }]; a cell is
 * a value or { text, tone?, href?, title?, sort? }.
 */
export const table = (title, columns, rows, note) => ({ type: 'table', title, columns, rows, note });

/** Pass/fail lines. items: [{ label, ok: true | false | null, detail? }] */
export const checks = (title, items, note) => ({ type: 'checks', title, items, note });

/** Another page shown in place (a tool's own HTML report), with a link to open it alone. */
export const embed = (title, src, note) => ({ type: 'embed', title, src, note });

/** Links to more files. items: [{ label, href, note? }] */
export const links = (title, items) => ({ type: 'links', title, items });

/** A highlighted line of text. */
export const note = (text, tone = 'neutral') => ({ type: 'note', text, tone });

/** good when value ≥ good, warn when ≥ warn, else bad (higher is better). */
export function tone(value, good, warn) {
	if (value === null || value === undefined || Number.isNaN(value)) return 'neutral';
	return value >= good ? 'good' : value >= warn ? 'warn' : 'bad';
}

/** good when value ≤ good, warn when ≤ warn, else bad (lower is better). */
export function toneBelow(value, good, warn) {
	if (value === null || value === undefined || Number.isNaN(value)) return 'neutral';
	return value <= good ? 'good' : value <= warn ? 'warn' : 'bad';
}

/** 0 is good, anything else bad: findings that fail CI. */
export const toneZero = (count) => (count === 0 ? 'good' : 'bad');

export const pct = (share, digits = 0) => (share === null || share === undefined ? '–' : `${(share * 100).toFixed(digits)}%`);
export const ms = (value) => (value >= 1000 ? `${(value / 1000).toFixed(2)} s` : `${value < 10 ? value.toFixed(1) : Math.round(value)} ms`);
export const kb = (bytes) => `${(bytes / 1024).toFixed(bytes < 10240 ? 1 : 0)} KB`;
export const mb = (bytes) => `${(bytes / 1048576).toFixed(1)} MB`;
export const count = (n) => n.toLocaleString('en-US');

/** A coverage cell: the share, toned at 80% and 60%. */
export function coverageCell(covered, total) {
	if (!total) return { text: '–', sort: -1 };
	const share = covered / total;
	return { text: pct(share), tone: tone(share, 0.8, 0.6), sort: share, title: `${covered} of ${total}` };
}

/** Plural noun: plural(3, 'test') → '3 tests'. */
export const plural = (n, word, many = `${word}s`) => `${count(n)} ${n === 1 ? word : many}`;

/** [[key, items]] grouped by keyOf, largest group first. */
export function groupBy(items, keyOf) {
	const groups = new Map();
	for (const item of items) {
		const key = keyOf(item);
		groups.set(key, [...(groups.get(key) || []), item]);
	}
	return [...groups].sort((a, b) => b[1].length - a[1].length);
}
