/**
 * Presentation of /api/stats results: the daemon aggregates, this only lays buckets out for the
 * charts (empty days and hours filled in, weekday rows in the week_start order, bucket labels) and
 * derives a few ratios. Pure, tested in series.test.ts.
 */
import type { StatsBucket, Telemetry } from '#lib/api/types.ts';
import { weekdays, type WeekStart } from '#lib/insights/week.ts';
import { dayKey } from '#lib/shared/format.ts';

const pad = (n: number) => String(n).padStart(2, '0');

// ---- time buckets ------------------------------------------------------------------------

export type Slot = {
	/** The bucket key: YYYY-MM-DD, or "YYYY-MM-DD HH" for clock hours. */
	key: string;
	/** Local start of the slot. */
	date: Date;
	/** The daemon's bucket, or null when nothing happened then. */
	bucket: StatsBucket | null;
};

/** Key of the local clock hour of `date`, as GROUPS.day_hour writes it. */
export const hourKey = (date: Date) => `${dayKey(date)} ${pad(date.getHours())}`;

/**
 * Every local day (or clock hour, `hourly`) from `start` to `end`, each with its bucket, so quiet
 * days show as zeros. Bounded: never more than 800 slots, the newest kept.
 */
export function timeSlots(groups: StatsBucket[], start: Date, end: Date, hourly: boolean): Slot[] {
	const byKey = new Map(groups.map((g) => [g.key, g]));
	const cursor = new Date(start);
	if (hourly) cursor.setMinutes(0, 0, 0);
	else cursor.setHours(0, 0, 0, 0);
	const slots: Slot[] = [];
	while (cursor <= end) {
		const key = hourly ? hourKey(cursor) : dayKey(cursor);
		slots.push({ key, date: new Date(cursor), bucket: byKey.get(key) ?? null });
		if (hourly) cursor.setHours(cursor.getHours() + 1);
		else cursor.setDate(cursor.getDate() + 1);
	}
	return slots.slice(-800);
}

/** The instants a slot covers, for a link to its prompts (a day, or one clock hour). */
export function slotWindow(slot: Slot, hourly: boolean): { since: string; until: string } {
	const next = new Date(slot.date);
	if (hourly) next.setHours(next.getHours() + 1);
	else next.setDate(next.getDate() + 1);
	return { since: slot.date.toISOString(), until: next.toISOString() };
}

/** The earliest day to chart: the range's start, else (all time) the first prompt, else today. */
export function chartStart(start: Date | null, firstAt: string | null, now: number): Date {
	return start ?? new Date(firstAt ? Date.parse(firstAt) : now);
}

export type Streaks = {
	/** Days with at least one prompt. */
	active: number;
	/** Days in the window. */
	days: number;
	longest: number;
	/** Consecutive active days up to today (or yesterday, when today has nothing yet). */
	current: number;
};

/** Runs of consecutive days with prompts, over day slots (oldest first). */
export function streaks(slots: Slot[]): Streaks {
	let longest = 0;
	let run = 0;
	for (const s of slots) {
		run = s.bucket?.prompts ? run + 1 : 0;
		longest = Math.max(longest, run);
	}
	let current = 0;
	const last = slots.length - 1;
	// Today may not have started yet: a streak through yesterday still counts.
	const from = last >= 0 && !slots[last].bucket?.prompts ? last - 1 : last;
	for (let i = from; i >= 0 && slots[i].bucket?.prompts; i--) current++;
	return {
		active: slots.filter((s) => s.bucket?.prompts).length,
		days: slots.length,
		longest,
		current
	};
}

/**
 * A weekday × hour grid of a measure (`weekday_hour` buckets, keys "1-09", ISO weekdays), rows in
 * the week_start order.
 */
export function weekGrid(
	groups: StatsBucket[],
	start: WeekStart,
	measure: (b: StatsBucket) => number = (b) => b.prompts
): { rows: ReturnType<typeof weekdays>; isoDays: number[]; values: number[][] } {
	const isoDays = start === 'sunday' ? [7, 1, 2, 3, 4, 5, 6] : [1, 2, 3, 4, 5, 6, 7];
	const values = isoDays.map(() => Array<number>(24).fill(0));
	for (const g of groups) {
		const [day, hour] = (g.key ?? '').split('-').map(Number);
		const row = isoDays.indexOf(day);
		if (row >= 0 && hour >= 0 && hour < 24) values[row][hour] = measure(g);
	}
	return { rows: weekdays(start), isoDays, values };
}

// ---- size and length buckets -------------------------------------------------------------

/** Lower bounds of the daemon's `size` buckets, in typed characters (storage/store.py SIZES). */
export const SIZES = [0, 20, 80, 200, 500, 1500];
/** Lower bounds of the daemon's `session_length` buckets, in seconds (storage/stats.py). */
export const SESSION_LENGTHS = [0, 60, 300, 900, 1800, 3600, 7200, 14400];

export type Band = { key: string; label: string; tick: string; bucket: StatsBucket | null };

/** The zero-padded key the daemon gives a bucket's lower bound. */
export const boundKey = (bound: number) => String(bound).padStart(5, '0');

/** Every bucket of a banded grouping, empty ones included, labelled "20–79" style by `unit`. */
export function bands(
	groups: StatsBucket[],
	bounds: number[],
	unit: (n: number) => string
): Band[] {
	const byKey = new Map(groups.map((g) => [g.key, g]));
	return bounds.map((lower, i) => {
		const upper = bounds[i + 1];
		const tick =
			upper === undefined ? `${unit(lower)}+` : i === 0 ? `<${unit(upper)}` : unit(lower);
		const label =
			upper === undefined
				? `${unit(lower)} or more`
				: i === 0
					? `Under ${unit(upper)}`
					: `${unit(lower)} to ${unit(upper)}`;
		return { key: boundKey(lower), label, tick, bucket: byKey.get(boundKey(lower)) ?? null };
	});
}

/** "30s", "15m", "2h": a session length bound, short enough for an axis tick. */
export const lengthUnit = (seconds: number) =>
	seconds < 60
		? `${seconds}s`
		: seconds < 3600
			? `${Math.round(seconds / 60)}m`
			: `${Math.round(seconds / 3600)}h`;

// ---- ratios ------------------------------------------------------------------------------

/** a / b, or null when b is 0 (a share that can't be computed is not 0 %). */
export const ratio = (a: number, b: number): number | null => (b ? a / b : null);

/** Share of the prompt-side tokens served from cache, null without telemetry. */
export function cacheShare(b: StatsBucket): number | null {
	return ratio(b.cache_read_tokens, b.input_tokens + b.cache_read_tokens + b.cache_creation_tokens);
}

export const tokenTotal = (b: StatsBucket) =>
	b.input_tokens + b.output_tokens + b.cache_read_tokens + b.cache_creation_tokens;

/** Prompts that sent a typed text sent before (case and spacing aside). */
export const repeated = (b: StatsBucket) => Math.max(0, b.prompts - b.distinct_texts);

// ---- telemetry ---------------------------------------------------------------------------

export const METRICS = {
	linesAdded: 'claude_code.lines_of_code.count[added]',
	linesRemoved: 'claude_code.lines_of_code.count[removed]',
	commits: 'claude_code.commit.count',
	pullRequests: 'claude_code.pull_request.count',
	activeUser: 'claude_code.active_time.total[user]',
	activeCli: 'claude_code.active_time.total[cli]'
} as const;

/** A metric total, or null when telemetry never reported it. */
export const metric = (t: Telemetry, name: keyof typeof METRICS): number | null =>
	t.metrics[METRICS[name]] ?? null;

/** True when any telemetry arrived for these sessions. */
export const hasTelemetry = (t: Telemetry) => Object.keys(t.metrics).length > 0;

// ---- who the model worked for ------------------------------------------------------------

// Drawn in this order: teal | yellow | iris is validated as adjacent pairs (dataviz validator, both modes).
export const WORK = ['main', 'subagent', 'system'] as const;
export type Work = (typeof WORK)[number];

export const WORK_INFO: Record<Work, { label: string; hint: string; color: string }> = {
	main: {
		label: 'Answering you',
		hint: 'The main thread working on your prompts.',
		color: 'var(--chart-1)'
	},
	subagent: {
		label: 'Subagents',
		hint: 'Agents the main thread delegated to.',
		color: 'var(--chart-6)'
	},
	system: {
		label: 'System calls',
		hint: 'Calls the tool makes on its own: compaction, titles, summaries, suggestions, hook prompts.',
		color: 'var(--chart-2)'
	}
};

/** "agent:builtin:Explore" -> "Explore"; other query sources in words ("away summary"). */
export function querySourceLabel(source: string | null): string {
	if (!source) return 'Main thread';
	if (source.startsWith('agent:')) return source.split(':').at(-1) || source;
	return source.replaceAll('_', ' ').replace(/^./, (c) => c.toUpperCase());
}
