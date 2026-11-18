/**
 * One sentence that opens each Overview tab (lib/insights/tabs.ts), said from that tab's own
 * numbers: when you prompt, who wrote the prompts, where the money went, what the agent did, what
 * you starred. Each function returns null when its numbers say nothing worth a sentence; none
 * guesses a figure the daemon did not send. Pure, tested in takeaways.test.ts.
 */
import type { Stats, StatsBucket, Telemetry } from '#lib/api/types.ts';
import type { Overview } from '#lib/insights/overview.ts';
import { bands, lengthUnit, repeated, SESSION_LENGTHS, weekGrid } from '#lib/insights/series.ts';
import type { OverviewTab } from '#lib/insights/tabs.ts';
import { asKind, KIND_INFO, TOOL_KINDS } from '#lib/insights/tools.ts';
import { weekdays } from '#lib/insights/week.ts';
import { ORIGINS } from '#lib/prompts/origin.ts';
import { modelName, money, percent, plural } from '#lib/shared/format.ts';

/** Fewer prompts than this in the week grid, and a "peak" is noise. */
export const PEAK_MINIMUM = 10;
/** Hours in the peak window ("14:00–18:00"). */
export const PEAK_HOURS = 4;
/** A neighbouring day this close to the busiest one shares the peak ("Wed–Thu"). */
export const CLOSE_DAY = 0.8;

const sentence = (parts: (string | null)[]): string | null => {
	const said = parts.filter((p): p is string => !!p);
	if (!said.length) return null;
	const text = said.join('; ');
	return `${text.charAt(0).toUpperCase()}${text.slice(1)}.`;
};

const hour = (h: number) => `${String(h).padStart(2, '0')}:00`;

const top = <T>(items: T[], measure: (item: T) => number): T | undefined =>
	items.reduce<T | undefined>(
		(best, x) => (!best || measure(x) > measure(best) ? x : best),
		undefined
	);

const sum = (values: number[]) => values.reduce((a, b) => a + b, 0);

// ---- Activity ------------------------------------------------------------------------------------

/** The busiest day, with its neighbour when that one is nearly as busy: "Wed" or "Wed–Thu". */
function peakDays(values: number[][]): string {
	const days = values.map((row) => sum(row));
	const names = weekdays('monday').map((d) => d.short);
	const best = days.indexOf(Math.max(...days));
	const near = [best - 1, best + 1].filter(
		(i) => i >= 0 && i < 7 && days[i] >= CLOSE_DAY * days[best]
	);
	const next = top(near, (i) => days[i]);
	if (next === undefined) return names[best];
	const [a, b] = [Math.min(best, next), Math.max(best, next)];
	return `${names[a]}–${names[b]}`;
}

/** The `PEAK_HOURS` window of the day with the most prompts, over every day: "14:00–18:00". */
function peakHours(values: number[][]): string {
	const hours = Array.from({ length: 24 }, (_, h) => sum(values.map((row) => row[h])));
	const starts = Array.from({ length: 24 - PEAK_HOURS + 1 }, (_, s) => s);
	const start = top(starts, (s) => sum(hours.slice(s, s + PEAK_HOURS))) ?? 0;
	return `${hour(start)}–${hour(start + PEAK_HOURS)}`;
}

/** "Peak: Wed–Thu 14:00–18:00", from the weekday × hour grid; null with too few prompts. */
export function peak(week: Stats): string | null {
	if (week.totals.prompts < PEAK_MINIMUM) return null;
	const { values } = weekGrid(week.groups, 'monday');
	return `peak: ${peakDays(values)} ${peakHours(values)}`;
}

/** "a typical session lasts 15m to 30m": the length band that holds the median session. */
export function typicalSession(lengths: Stats): string | null {
	const list = bands(lengths.groups, SESSION_LENGTHS, lengthUnit);
	const total = sum(list.map((b) => b.bucket?.sessions ?? 0));
	if (!total) return null;
	let seen = 0;
	const middle = list.find((b) => (seen += b.bucket?.sessions ?? 0) >= total / 2);
	return middle ? `a typical session lasts ${middle.label.toLowerCase()}` : null;
}

export const activityTakeaway = (o: Pick<Overview, 'week' | 'sessionLengths'>): string | null =>
	sentence([peak(o.week), typicalSession(o.sessionLengths)]);

// ---- Prompt types --------------------------------------------------------------------------------

const ORIGIN_LIST = Object.entries(ORIGINS);

/** Typed by you (not by the tool or its agents); an origin this dashboard does not know is not. */
const isYours = (g: StatsBucket) =>
	ORIGIN_LIST.some(([key, info]) => key === g.key && !info.system);

/** "57% of prompts came from you": typed and slash commands, over every origin. */
function yourShare(origins: Stats): string | null {
	const all = origins.totals.prompts;
	if (!all) return null;
	const yours = sum(origins.groups.filter(isYours).map((g) => g.prompts));
	return `${percent(yours / all)} of prompts came from you`;
}

/** The slash command run most: from telemetry when a tool reports it, else from the prompts. */
function mostUsedCommand(telemetry: Telemetry, commands: Stats): string | null {
	const used = top(telemetry.commands, (c) => c.uses)?.command;
	const prompted = top(
		commands.groups.filter((g) => g.key),
		(g) => g.prompts
	)?.key;
	const command = used ?? prompted;
	return command ? `${command} is the command you run most` : null;
}

function sentAgain(totals: StatsBucket): string | null {
	const again = repeated(totals);
	return again ? `${plural(again, 'prompt')} repeated a text sent before` : null;
}

export const typesTakeaway = (
	o: Pick<Overview, 'origins' | 'commands' | 'telemetry' | 'activity'>
): string | null =>
	sentence([
		yourShare(o.origins),
		sentAgain(o.activity.totals),
		mostUsedCommand(o.telemetry, o.commands)
	]);

// ---- Models and cost -----------------------------------------------------------------------------

/**
 * The main model and its share: of the spend when costs are reported, else of the model requests,
 * else of the prompts. Null when no model is named.
 */
export function mainModel(models: Stats): string | null {
	const named = models.groups.filter((g) => g.key);
	const t = models.totals;
	const measure: [(b: StatsBucket) => number, (lead: string, share: string) => string][] = [
		[(b) => b.cost_usd, (lead, share) => `${lead} took ${share} of the ${money(t.cost_usd)} spent`],
		[(b) => b.api_requests, (lead, share) => `${lead} answered ${share} of the model requests`],
		[(b) => b.prompts, (lead, share) => `${lead} answered ${share} of the prompts`]
	];
	for (const [value, say] of measure) {
		const all = value(t);
		const lead = top(named, value);
		if (all > 0 && lead && value(lead) > 0)
			return say(modelName(lead.key ?? ''), percent(value(lead) / all));
	}
	return null;
}

export const costTakeaway = (o: Pick<Overview, 'models'>): string | null =>
	sentence([mainModel(o.models)]);

// ---- Agentic work --------------------------------------------------------------------------------

const ENDED = ['done', 'failed', 'interrupted', 'superseded'] as const;

/** "82% of finished turns were done": of the turns that ended, the share the agent completed. */
function finishedShare(outcomes: Stats): string | null {
	const count = (key: string) => outcomes.groups.find((g) => g.key === key)?.prompts ?? 0;
	const ended = sum(ENDED.map(count));
	return ended ? `${percent(count('done') / ended)} of the turns that ended were done` : null;
}

/** "the agent made 9,234 tool calls, 47% read & search": the calls and the kind most of them were. */
function toolMix(toolKinds: Stats): string | null {
	const kinds = TOOL_KINDS.map((kind) => ({
		kind,
		calls: sum(toolKinds.groups.filter((g) => asKind(g.key) === kind).map((g) => g.tool_calls))
	}));
	const calls = sum(kinds.map((k) => k.calls));
	const lead = top(kinds, (k) => k.calls);
	if (!calls || !lead) return null;
	const share = `${percent(lead.calls / calls)} ${KIND_INFO[lead.kind].label.toLowerCase()}`;
	return `the agent made ${plural(calls, 'tool call')}, ${share}`;
}

export const agenticTakeaway = (o: Pick<Overview, 'outcomes' | 'toolKinds'>): string | null =>
	sentence([toolMix(o.toolKinds), finishedShare(o.outcomes)]);

// ---- Starred -------------------------------------------------------------------------------------

/**
 * How many prompts you starred in the window. The Overview loads at most `limit` of them, so at
 * the limit it says how many it shows, not how many there are.
 */
export function starredTakeaway(count: number, limit: number): string | null {
	if (!count) return null;
	if (count >= limit)
		return `Your ${plural(count, 'latest starred prompt')}; All starred lists every one.`;
	return `${plural(count, 'prompt')} starred in this window.`;
}

// ---- every tab -----------------------------------------------------------------------------------

/** The sentence that opens `tab`, or null when its numbers say nothing worth one. */
export function takeaway(tab: OverviewTab, o: Overview, starredLimit: number): string | null {
	switch (tab) {
		case 'activity':
			return activityTakeaway(o);
		case 'types':
			return typesTakeaway(o);
		case 'cost':
			return costTakeaway(o);
		case 'agentic':
			return agenticTakeaway(o);
		case 'starred':
			return starredTakeaway(o.starred.length, starredLimit);
	}
}
