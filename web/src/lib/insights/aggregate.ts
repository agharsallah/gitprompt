/**
 * Stories told from a list of prompts, computed in the browser from GET /api/prompts (the values
 * are already derived on read by the daemon; this only groups and sums them). Pure functions over
 * Prompt[], so every chart on Overview, Sessions and Projects is unit tested in aggregate.test.ts.
 */
import type { Prompt } from '#lib/api/types.ts';
import { dayKey } from '#lib/shared/format.ts';
import { outcome, supersededIds, type Outcome } from '#lib/insights/outcome.ts';
import { mergeProject, projectOf, type ProjectRef } from '#lib/insights/project.ts';
import { byKind, toolKind, type ToolKind } from '#lib/insights/tools.ts';

// ---- small numeric helpers ----------------------------------------------------------------

export const sum = (values: number[]) => values.reduce((a, b) => a + b, 0);

/** Linear-interpolated quantile of the finite values; null when there are none. */
export function quantile(values: (number | null | undefined)[], q: number): number | null {
	const xs = values.filter((v): v is number => typeof v === 'number' && Number.isFinite(v));
	if (!xs.length) return null;
	xs.sort((a, b) => a - b);
	const pos = (xs.length - 1) * q;
	const lo = Math.floor(pos);
	const hi = Math.ceil(pos);
	return xs[lo] + (xs[hi] - xs[lo]) * (pos - lo);
}

export const median = (values: (number | null | undefined)[]) => quantile(values, 0.5);

const n = (v: number | undefined | null) => (typeof v === 'number' && Number.isFinite(v) ? v : 0);

/** Sum of a `{name: count}` map. */
const total = (m: Record<string, number> | undefined) => sum(Object.values(m ?? {}));

// ---- tokens ------------------------------------------------------------------------------

export type Tokens = { input: number; output: number; cacheRead: number; cacheWrite: number };

export function tokensOf(p: Prompt): Tokens {
	const s = p.summary;
	return {
		input: n(s.input_tokens),
		output: n(s.output_tokens),
		cacheRead: n(s.cache_read_tokens),
		cacheWrite: n(s.cache_creation_tokens)
	};
}

export function addTokens(a: Tokens, b: Tokens): Tokens {
	return {
		input: a.input + b.input,
		output: a.output + b.output,
		cacheRead: a.cacheRead + b.cacheRead,
		cacheWrite: a.cacheWrite + b.cacheWrite
	};
}

export const tokenTotal = (t: Tokens) => t.input + t.output + t.cacheRead + t.cacheWrite;

/** Share of the prompt-side tokens served from cache (0..1), null without telemetry. */
export function cacheShare(t: Tokens): number | null {
	const prompt = t.input + t.cacheRead + t.cacheWrite;
	return prompt ? t.cacheRead / prompt : null;
}

const ZERO: Tokens = { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 };

// ---- headline totals ---------------------------------------------------------------------

export type Totals = {
	prompts: number;
	sessions: number;
	projects: number;
	cost: number;
	tokens: Tokens;
	/** Prompts that have usage telemetry (api_request events). */
	withTelemetry: number;
	outcomes: Record<Outcome, number>;
	responseMedian: number | null;
	responseP90: number | null;
	/** Median think time between two prompts of a session. */
	thinkMedian: number | null;
	toolCalls: number;
	toolFailures: number;
	filesWritten: number;
	commands: number;
	words: number;
};

export function totals(prompts: Prompt[], now = Date.now()): Totals {
	const superseded = supersededIds(prompts);
	const outcomes: Record<Outcome, number> = { done: 0, failed: 0, running: 0, open: 0 };
	const files = new Set<string>();
	for (const p of prompts) {
		outcomes[outcome(p, now, superseded.has(p.id))]++;
		for (const f of p.summary.files_written ?? []) files.add(f);
	}
	return {
		prompts: prompts.length,
		sessions: new Set(prompts.map((p) => p.fields.session_id ?? `#${p.id}`)).size,
		projects: new Set(prompts.map((p) => projectOf(p).key)).size,
		cost: sum(prompts.map((p) => n(p.summary.cost_usd))),
		tokens: prompts.map(tokensOf).reduce(addTokens, ZERO),
		withTelemetry: prompts.filter((p) => n(p.summary.api_requests) > 0).length,
		outcomes,
		responseMedian: median(prompts.map((p) => p.summary.response_s)),
		responseP90: quantile(
			prompts.map((p) => p.summary.response_s),
			0.9
		),
		thinkMedian: median(prompts.map((p) => p.summary.since_previous_prompt_s)),
		toolCalls: sum(prompts.map((p) => total(p.summary.tool_calls))),
		toolFailures: sum(prompts.map((p) => n(p.summary.tool_failures))),
		filesWritten: files.size,
		commands: sum(prompts.map((p) => p.summary.bash_commands?.length ?? 0)),
		words: sum(prompts.map((p) => n(p.stats.words)))
	};
}

// ---- over time ---------------------------------------------------------------------------

export type Day = { key: string; date: Date; prompts: number; cost: number; failed: number };

/** One entry per local calendar day from `start` to `end` inclusive, empty days included. */
export function byDay(prompts: Prompt[], start: Date, end: Date): Day[] {
	const days = new Map<string, Day>();
	const cursor = new Date(start.getFullYear(), start.getMonth(), start.getDate());
	const last = dayKey(end);
	// Bounded walk: never more than ~2 years of days, whatever the inputs.
	for (let i = 0; i < 800; i++) {
		const key = dayKey(cursor);
		days.set(key, { key, date: new Date(cursor), prompts: 0, cost: 0, failed: 0 });
		if (key === last) break;
		cursor.setDate(cursor.getDate() + 1);
	}
	for (const p of prompts) {
		const day = days.get(dayKey(p.received_at));
		if (!day) continue;
		day.prompts++;
		day.cost += n(p.summary.cost_usd);
		if (p.summary.ended_by === 'StopFailure') day.failed++;
	}
	return [...days.values()];
}

/** One entry per local clock hour from `start` to `end`: the 24-hour view's columns. */
export function byHour(prompts: Prompt[], start: Date, end: Date): Day[] {
	const hours: Day[] = [];
	const cursor = new Date(start);
	cursor.setMinutes(0, 0, 0);
	for (let i = 0; i < 24 * 8 && cursor <= end; i++) {
		hours.push({ key: cursor.toISOString(), date: new Date(cursor), prompts: 0, cost: 0, failed: 0 });
		cursor.setHours(cursor.getHours() + 1);
	}
	for (const p of prompts) {
		const t = Date.parse(p.received_at);
		const i = hours.findLastIndex((h) => h.date.getTime() <= t);
		if (i < 0 || t > end.getTime()) continue;
		hours[i].prompts++;
		hours[i].cost += n(p.summary.cost_usd);
		if (p.summary.ended_by === 'StopFailure') hours[i].failed++;
	}
	return hours;
}

/** Prompt counts by weekday (rows, Monday first) and local hour (columns). */
export function weekHours(prompts: Prompt[]): number[][] {
	const grid = Array.from({ length: 7 }, () => Array<number>(24).fill(0));
	for (const p of prompts) {
		const d = new Date(p.received_at);
		grid[(d.getDay() + 6) % 7][d.getHours()]++;
	}
	return grid;
}

// ---- what the agent did ------------------------------------------------------------------

export type ToolUse = { name: string; kind: ToolKind; count: number };

/** Every tool by number of calls, most used first. */
export function toolUse(prompts: Prompt[]): ToolUse[] {
	const counts = new Map<string, number>();
	for (const p of prompts)
		for (const [name, c] of Object.entries(p.summary.tool_calls ?? {}))
			counts.set(name, (counts.get(name) ?? 0) + c);
	return [...counts]
		.map(([name, count]) => ({ name, kind: toolKind(name), count }))
		.sort((a, b) => b.count - a.count || a.name.localeCompare(b.name));
}

/** Tool calls folded into kinds, in kind order. */
export function kindMix(prompts: Prompt[]) {
	const all: Record<string, number> = {};
	for (const t of toolUse(prompts)) all[t.name] = t.count;
	return byKind(all);
}

/** API requests per model, most used first. */
export function modelMix(prompts: Prompt[]): { model: string; requests: number }[] {
	const counts = new Map<string, number>();
	for (const p of prompts)
		for (const [model, c] of Object.entries(p.summary.models ?? {}))
			counts.set(model, (counts.get(model) ?? 0) + c);
	return [...counts]
		.map(([model, requests]) => ({ model, requests }))
		.sort((a, b) => b.requests - a.requests);
}

// ---- how prompts are written -------------------------------------------------------------

export const LENGTH_BUCKETS = [
	{ label: '1–5', max: 5 },
	{ label: '6–15', max: 15 },
	{ label: '16–40', max: 40 },
	{ label: '41–100', max: 100 },
	{ label: '101–250', max: 250 },
	{ label: '250+', max: Infinity }
] as const;

export type LengthBucket = {
	label: string;
	prompts: number;
	/** Median seconds the tool worked on prompts of this length. */
	medianResponse: number | null;
	/** Median tool calls they led to. */
	medianTools: number | null;
};

/** Prompt length (words) against how much work followed. */
export function lengthVsWork(prompts: Prompt[]): LengthBucket[] {
	return LENGTH_BUCKETS.map((b, i) => {
		const min = i ? LENGTH_BUCKETS[i - 1].max : 0;
		const inside = prompts.filter((p) => n(p.stats.words) > min && n(p.stats.words) <= b.max);
		return {
			label: b.label,
			prompts: inside.length,
			medianResponse: median(inside.map((p) => p.summary.response_s)),
			medianTools: median(inside.map((p) => total(p.summary.tool_calls)))
		};
	});
}

export type Craft = {
	medianWords: number | null;
	withCode: number;
	withQuestion: number;
	withLink: number;
	withMention: number;
	slashCommands: { name: string; count: number }[];
};

export function craft(prompts: Prompt[]): Craft {
	const slash = new Map<string, number>();
	for (const p of prompts) {
		const cmd = p.summary.slash_command ?? p.stats.slash_command;
		if (cmd) slash.set(cmd, (slash.get(cmd) ?? 0) + 1);
	}
	return {
		medianWords: median(prompts.map((p) => p.stats.words)),
		withCode: prompts.filter((p) => n(p.stats.code_blocks) > 0).length,
		withQuestion: prompts.filter((p) => n(p.stats.questions) > 0).length,
		withLink: prompts.filter((p) => p.stats.urls?.length).length,
		withMention: prompts.filter((p) => p.stats.at_mentions?.length).length,
		slashCommands: [...slash]
			.map(([name, count]) => ({ name, count }))
			.sort((a, b) => b.count - a.count)
	};
}

// ---- needs a look ------------------------------------------------------------------------

export type Attention = {
	secrets: Prompt[];
	failed: Prompt[];
	toolFailures: Prompt[];
	denied: Prompt[];
};

export function attention(prompts: Prompt[]): Attention {
	return {
		secrets: prompts.filter((p) => p.stats.secrets?.length),
		failed: prompts.filter((p) => p.summary.ended_by === 'StopFailure'),
		toolFailures: prompts.filter((p) => n(p.summary.tool_failures) > 0),
		denied: prompts.filter(
			(p) => p.summary.permission_denied?.length || n(p.summary.tool_rejections) > 0
		)
	};
}

// ---- sessions ----------------------------------------------------------------------------

export type SessionSummary = {
	id: string;
	project: ProjectRef;
	/** Oldest first, as the conversation went. */
	prompts: Prompt[];
	title: string | null;
	branch: string | null;
	startedAt: string;
	/** When the last turn ended (or was sent, if it has no end). */
	endedAt: string;
	cost: number;
	tokens: Tokens;
	failed: number;
	toolCalls: number;
	filesWritten: number;
	models: string[];
	permissionModes: string[];
};

const endOf = (p: Prompt) =>
	new Date(Date.parse(p.received_at) + n(p.summary.response_s) * 1000).toISOString();

/** Prompts grouped by session, most recently active first. */
export function sessions(prompts: Prompt[]): SessionSummary[] {
	const groups = new Map<string, Prompt[]>();
	for (const p of prompts) {
		const id = p.fields.session_id ?? `prompt-${p.id}`;
		groups.set(id, [...(groups.get(id) ?? []), p]);
	}
	return [...groups]
		.map(([id, list]) => {
			const ordered = [...list].sort((a, b) => a.id - b.id);
			const first = ordered[0];
			const last = ordered.at(-1)!;
			const files = new Set(ordered.flatMap((p) => p.summary.files_written ?? []));
			const models = new Set(ordered.flatMap((p) => Object.keys(p.summary.models ?? {})));
			const modes = new Set(
				ordered.map((p) => p.fields.permission_mode).filter((m): m is string => !!m)
			);
			return {
				id,
				project: ordered.map(projectOf).reduce(mergeProject),
				prompts: ordered,
				title:
					[...ordered].reverse().find((p) => p.fields.session_title)?.fields.session_title ??
					null,
				branch: last.context.git_branch ?? null,
				startedAt: first.received_at,
				endedAt: endOf(last),
				cost: sum(ordered.map((p) => n(p.summary.cost_usd))),
				tokens: ordered.map(tokensOf).reduce(addTokens, ZERO),
				failed: ordered.filter((p) => p.summary.ended_by === 'StopFailure').length,
				toolCalls: sum(ordered.map((p) => total(p.summary.tool_calls))),
				filesWritten: files.size,
				models: [...models],
				permissionModes: [...modes]
			};
		})
		.sort((a, b) => Date.parse(b.endedAt) - Date.parse(a.endedAt));
}

// ---- projects ----------------------------------------------------------------------------

export type ProjectSummary = {
	project: ProjectRef;
	prompts: number;
	sessions: number;
	cost: number;
	lastAt: string;
	branches: string[];
	filesWritten: number;
	failed: number;
	kinds: { kind: ToolKind; count: number }[];
	/** Prompts per day over the window passed to projects(). */
	daily: number[];
};

/** Prompts grouped by project, most prompts first. `days` sets the daily sparkline's window. */
export function projects(prompts: Prompt[], days: Day[] = []): ProjectSummary[] {
	const groups = new Map<string, Prompt[]>();
	for (const p of prompts) {
		const key = projectOf(p).key;
		groups.set(key, [...(groups.get(key) ?? []), p]);
	}
	const index = new Map(days.map((d, i) => [d.key, i]));
	return [...groups.values()]
		.map((list) => {
			const daily = Array<number>(days.length).fill(0);
			for (const p of list) {
				const i = index.get(dayKey(p.received_at));
				if (i !== undefined) daily[i]++;
			}
			return {
				project: list.map(projectOf).reduce(mergeProject),
				prompts: list.length,
				sessions: new Set(list.map((p) => p.fields.session_id)).size,
				cost: sum(list.map((p) => n(p.summary.cost_usd))),
				lastAt: list.map((p) => p.received_at).sort().at(-1)!,
				branches: [
					...new Set(list.map((p) => p.context.git_branch).filter((b): b is string => !!b))
				],
				filesWritten: new Set(list.flatMap((p) => p.summary.files_written ?? [])).size,
				failed: list.filter((p) => p.summary.ended_by === 'StopFailure').length,
				kinds: kindMix(list),
				daily
			};
		})
		.sort((a, b) => b.prompts - a.prompts || b.lastAt.localeCompare(a.lastAt));
}

// ---- activity lanes ----------------------------------------------------------------------

export type Lane = {
	key: string;
	label: string;
	root?: string;
	marks: { id: number; at: number; outcome: Outcome; text: string }[];
};

/**
 * One lane per project (the `max - 1` busiest, the rest folded into "Other projects"), each with a
 * mark per prompt: the overview's picture of where and when you worked.
 */
export function lanes(prompts: Prompt[], max = 6, now = Date.now()): Lane[] {
	const superseded = supersededIds(prompts);
	const ranked = projects(prompts);
	const keep = ranked.length > max ? ranked.slice(0, max - 1) : ranked;
	const kept = new Set(keep.map((r) => r.project.key));
	const lane = new Map<string, Lane>(
		keep.map((r) => [
			r.project.key,
			{ key: r.project.key, label: r.project.name, root: r.project.root, marks: [] }
		])
	);
	const other: Lane = { key: '(other)', label: 'Other projects', marks: [] };
	for (const p of prompts) {
		const key = projectOf(p).key;
		(kept.has(key) ? lane.get(key)! : other).marks.push({
			id: p.id,
			at: Date.parse(p.received_at),
			outcome: outcome(p, now, superseded.has(p.id)),
			text: p.prompt ?? ''
		});
	}
	const out = [...lane.values()];
	if (other.marks.length) out.push(other);
	for (const l of out) l.marks.sort((a, b) => a.at - b.at);
	return out;
}
