/**
 * The week strip on the Overview's first screen: each day (or clock hour, over 24 hours) of the
 * range as one column stacked by project, from /api/stats `group_by=day&split_by=repo`. The
 * projects with the most prompts get a color each, the rest share "Other projects". A day that
 * stands out gets a mark. Pure, tested in week-strip.test.ts.
 */
import type { StatsBucket } from '#lib/api/types.ts';
import { slotNames, type Slot, type Streaks } from '#lib/insights/series.ts';
import { folder, money, plural } from '#lib/shared/format.ts';
import { sessionsHref } from '#lib/shared/links.ts';
import type { Series, StackedColumn, StackPart } from '#lib/ui/charts/stacked.ts';

/** Projects drawn in their own color; the others are stacked together. */
export const MAX_PROJECTS = 5;
/** A day stands out when its prompts reach this many times the median active day. */
export const STANDOUT_RATIO = 1.5;

export const OTHER = 'other-projects';
/** The series of prompts sent outside any project. */
export const NO_PROJECT = 'no-project';

const COLORS = ['--chart-1', '--chart-2', '--chart-3', '--chart-4', '--chart-5'];
const OTHER_COLOR = 'var(--chart-6)';

export type StripColumn = StackedColumn & {
	/** The local day it belongs to (YYYY-MM-DD): where it leads. */
	day: string;
	bucket: StatsBucket | null;
};

const seriesKey = (part: StatsBucket) => part.key ?? NO_PROJECT;
const projectName = (part: StatsBucket) =>
	part.detail ?? folder(part.key) ?? part.key ?? 'No project';

/** The projects of the period, most prompts first: the first MAX_PROJECTS, then "Other projects". */
export function stripSeries(groups: StatsBucket[], max = MAX_PROJECTS): Series[] {
	const totals = new Map<string, { label: string; prompts: number }>();
	for (const part of groups.flatMap((g) => g.parts ?? [])) {
		const key = seriesKey(part);
		const seen = totals.get(key) ?? { label: projectName(part), prompts: 0 };
		totals.set(key, { ...seen, prompts: seen.prompts + part.prompts });
	}
	const ranked = [...totals.entries()]
		.filter(([, t]) => t.prompts > 0)
		.sort(([, a], [, b]) => b.prompts - a.prompts || a.label.localeCompare(b.label));
	const series = ranked
		.slice(0, max)
		.map(([key, t], i) => ({ key, label: t.label, color: `var(${COLORS[i]})` }));
	return ranked.length > max
		? [...series, { key: OTHER, label: 'Other projects', color: OTHER_COLOR }]
		: series;
}

/** A day's prompts per series, in series order (a project not drawn on its own goes to Other). */
export function stripParts(bucket: StatsBucket | null, series: Series[]): StackPart[] {
	const values = new Map(series.map((s) => [s.key, 0]));
	for (const part of bucket?.parts ?? []) {
		const own = seriesKey(part);
		const key = values.has(own) ? own : OTHER;
		if (values.has(key)) values.set(key, (values.get(key) ?? 0) + part.prompts);
	}
	return series.map((s) => ({ series: s.key, value: values.get(s.key) ?? 0 }));
}

/** One column per slot, oldest first, stacked by `series`. */
export function stripColumns(
	slots: Slot[],
	series: Series[],
	hourly: boolean,
	now: Date
): StripColumn[] {
	return slots.map((slot) => ({
		key: slot.key,
		...slotNames(slot, hourly, now, slots.length),
		value: slot.bucket?.prompts ?? 0,
		parts: stripParts(slot.bucket, series),
		day: slot.key.slice(0, 10),
		bucket: slot.bucket
	}));
}

/** The middle value (the mean of the two middle ones for an even count); 0 for none. */
export function median(values: number[]): number {
	if (!values.length) return 0;
	const sorted = [...values].sort((a, b) => a - b);
	const mid = Math.floor(sorted.length / 2);
	return sorted.length % 2 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2;
}

export type Standout = { key: string; times: number };

/**
 * The column whose prompts are farthest above the median of the active columns (quiet days would
 * make any day stand out), when it reaches STANDOUT_RATIO times that median; else none.
 */
export function standout(columns: StackedColumn[]): Standout | null {
	const active = columns.filter((c) => c.value > 0);
	const middle = median(active.map((c) => c.value));
	if (!middle) return null;
	const top = active.reduce((best, c) => (c.value > best.value ? c : best), active[0]);
	const times = top.value / middle;
	return times >= STANDOUT_RATIO ? { key: top.key, times } : null;
}

/** The series with the most prompts in a column, or null for a quiet one. */
export function topSeries(column: StackedColumn, series: Series[]): Series | null {
	const best = column.parts.reduce<StackPart | null>(
		(top, p) => (p.value > (top?.value ?? 0) ? p : top),
		null
	);
	return series.find((s) => s.key === best?.series) ?? null;
}

/** What hovering a column says, line by line: prompts, spend (when reported), commits, top project. */
export function stripReadout(column: StripColumn, series: Series[], cost: boolean): string[] {
	const b = column.bucket;
	const top = topSeries(column, series);
	return [
		plural(column.value, 'prompt'),
		...(cost ? [`${money(b?.cost_usd ?? 0)} spent`] : []),
		plural(b?.git_commits ?? 0, 'commit'),
		...(top ? [`Mostly ${top.label}`] : [])
	];
}

/** "Active on 5 days of 7; longest streak 3 days, current 2 days." */
export function streakLine(run: Streaks): string {
	const current = run.current ? `, current ${plural(run.current, 'day')}` : '';
	return `Active on ${plural(run.active, 'day')} of ${run.days}; longest streak ${plural(run.longest, 'day')}${current}.`;
}

/** "Tuesday 3 November stands out: 42 prompts, 2.1× a usual day.", or null with no mark. */
export function standoutLine(
	columns: StackedColumn[],
	marked: Standout | null,
	hourly: boolean
): string | null {
	const column = marked && columns.find((c) => c.key === marked.key);
	if (!marked || !column) return null;
	const usual = hourly ? 'hour' : 'day';
	return `${column.label} stands out: ${plural(column.value, 'prompt')}, ${marked.times.toFixed(1)}× a usual ${usual}.`;
}

/** Where a column leads: the Sessions page on its day, in the Overview's project if one is chosen. */
export const dayHref = (column: StripColumn, project?: string) =>
	sessionsHref({ day: column.day, project });
