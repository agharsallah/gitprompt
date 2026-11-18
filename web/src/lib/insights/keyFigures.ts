/**
 * The five figures the Overview leads with, each with its change against the period before (in
 * words), its trend over the range and the prompts behind it. A figure no tool in use reports reads
 * "Not reported" and names who could, never 0. Pure, tested in keyFigures.test.ts; the panel
 * (panels/KeyFigures.svelte) only draws them.
 */
import type { Delta, RedonePart, RedoneWork, Stats, StatsBucket } from '#lib/api/types.ts';
import type { Doorway } from '#lib/insights/filters.ts';
import type { Range } from '#lib/insights/range.ts';
import type { Slot } from '#lib/insights/series.ts';
import { duration, folder, integer, money, percent, plural } from '#lib/shared/format.ts';
import type { Availability } from '#lib/sources/capabilities.ts';
import { whoCouldReport } from '#lib/sources/missing.ts';

export type FigureId = 'redone' | 'agent_time' | 'spend' | 'commits' | 'leaks';

export type Figure = {
	id: FigureId;
	label: string;
	value: string;
	hint?: string;
	/** "+18% vs the 7 days before". */
	change?: string;
	trend?: number[];
	href?: string;
	tone?: 'warning';
	/** The tooltip. */
	title?: string;
};

export type KeyFigureInput = {
	/** /api/stats with `key_figures` and `previous`. */
	stats: Stats;
	/** Projects by spend, most first. */
	repos: Stats;
	range: Range;
	slots: Slot[];
	link: Doorway;
	/** Where the possible leaks are listed. */
	leaksHref: string;
	can: Reporters;
};

/** Who reports cost, turn ends (agent time, redone turns) and tool calls (commits). */
export type Reporters = { cost: Availability; turnEnd: Availability; tools: Availability };

/** How a figure moved, in words: "+18% vs the 7 days before"; nothing without a period before. */
export function changeWords(d: Delta | undefined, range: Range): string | undefined {
	if (!d || range.days === null) return undefined;
	const before = `the ${range.label} before`;
	if (d.change === undefined) return d.before === 0 && d.now > 0 ? `none ${before}` : undefined;
	if (d.change === 0) return `same as ${before}`;
	return `${d.change > 0 ? '+' : '−'}${percent(Math.abs(d.change))} vs ${before}`;
}

/** The project that spent most, named as the Spend panel names it; none without spend. */
export function topProject(repos: Stats): { name: string; cost_usd: number } | undefined {
	const top = repos.groups.find((g) => g.cost_usd > 0);
	return top && { name: top.detail ?? folder(top.key) ?? 'No folder', cost_usd: top.cost_usd };
}

/** True when no tool in use reports the figure and none came in: "Not reported", not 0. */
const unreported = (a: Availability, value: number) => !a.available.length && value === 0;

function notReported(id: FigureId, label: string, what: string, a: Availability): Figure {
	return { id, label, value: 'Not reported', hint: whoCouldReport(what, a) };
}

const series = (slots: Slot[], measure: (b: StatsBucket) => number) =>
	slots.map((s) => (s.bucket ? measure(s.bucket) : 0));

// ---- redone work ------------------------------------------------------------------------------

const REASONS = ['failed', 'interrupted', 'reworded'] as const;

/** Cost when reported, else time: "$8.10" or "40m 00s". */
const amount = (p: RedonePart) =>
	p.cost_usd === undefined ? duration(p.agent_time_s) : money(p.cost_usd);

/** "$14.20 and 1h 12m redone", or the time alone without cost. */
export function redoneValue(r: RedoneWork): string {
	const time = duration(r.agent_time_s);
	return r.cost_usd === undefined ? `${time} redone` : `${money(r.cost_usd)} and ${time} redone`;
}

/** "failed $8.10 · reworded $4.10", and the tooltip with turns and time of each reason. */
function redoneSplit(r: RedoneWork): { hint: string; title: string } {
	const parts = REASONS.filter((why) => r[why].turns > 0);
	return {
		hint: parts.map((why) => `${why} ${amount(r[why])}`).join(' · '),
		title: parts
			.map((why) => {
				const p = r[why];
				const cost = p.cost_usd === undefined ? '' : `, ${money(p.cost_usd)}`;
				return `${why}: ${plural(p.turns, 'turn')}, ${duration(p.agent_time_s)}${cost}`;
			})
			.join('\n')
	};
}

function redoneFigure({ stats, range, link, can }: KeyFigureInput): Figure {
	const r = stats.redone;
	const label = 'Redone work';
	if (!r?.turns) {
		if (unreported(can.turnEnd, 0)) return notReported('redone', label, 'turn ends', can.turnEnd);
		return { id: 'redone', label, value: 'Nothing redone', hint: 'No turn to do again' };
	}
	const changes = stats.previous?.changes;
	return {
		id: 'redone',
		label,
		value: redoneValue(r),
		...redoneSplit(r),
		change: changeWords(
			r.cost_usd === undefined ? changes?.redone_agent_time_s : changes?.redone_cost_usd,
			range
		),
		tone: 'warning',
		// There is no filter for exactly the redone turns yet (a reworded turn is "done"): this
		// opens the failed and interrupted ones, most of what was redone.
		href: link({ outcome: ['failed', 'interrupted'] })
	};
}

// ---- the others ---------------------------------------------------------------------------------

function agentTimeFigure({ stats, range, slots, link, can }: KeyFigureInput): Figure {
	const t = stats.totals;
	const label = 'Agent working time';
	if (unreported(can.turnEnd, t.agent_time_s))
		return notReported('agent_time', label, 'turn ends', can.turnEnd);
	return {
		id: 'agent_time',
		label,
		value: duration(t.agent_time_s),
		hint: `over ${plural(t.ended, 'finished turn')}`,
		change: changeWords(stats.previous?.changes.agent_time_s, range),
		trend: series(slots, (b) => b.agent_time_s),
		href: link()
	};
}

function spendFigure({ stats, repos, range, slots, link, can }: KeyFigureInput): Figure {
	const t = stats.totals;
	if (unreported(can.cost, t.cost_usd)) return notReported('spend', 'Spend', 'cost', can.cost);
	const top = topProject(repos);
	return {
		id: 'spend',
		label: 'Spend',
		value: money(t.cost_usd),
		hint: top ? `most on ${top.name}: ${money(top.cost_usd)}` : undefined,
		change: changeWords(stats.previous?.changes.cost_usd, range),
		trend: series(slots, (b) => b.cost_usd),
		href: link()
	};
}

function commitsFigure({ stats, range, slots, link, can }: KeyFigureInput): Figure {
	const t = stats.totals;
	const label = 'Commits after prompts';
	if (unreported(can.tools, t.git_commits))
		return notReported('commits', label, 'tool calls', can.tools);
	return {
		id: 'commits',
		label,
		value: integer(t.git_commits),
		hint: `after ${plural(t.with_commit, 'prompt')}`,
		change: changeWords(stats.previous?.changes.git_commits, range),
		trend: series(slots, (b) => b.git_commits),
		href: link({ has: ['commit'] })
	};
}

function leaksFigure({ stats, range, leaksHref }: KeyFigureInput): Figure {
	const open = stats.possible_leaks ?? 0;
	return {
		id: 'leaks',
		label: 'Possible leaks',
		value: open ? integer(open) : 'None found',
		hint: open ? 'secrets or personal data in prompts, not handled yet' : undefined,
		change: changeWords(stats.previous?.changes.possible_leaks, range),
		tone: open ? 'warning' : undefined,
		href: leaksHref
	};
}

/** The five, in the order the Overview shows them. */
export function keyFigures(input: KeyFigureInput): Figure[] {
	return [redoneFigure, agentTimeFigure, spendFigure, commitsFigure, leaksFigure].map((f) =>
		f(input)
	);
}
