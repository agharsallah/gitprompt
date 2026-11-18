/**
 * The sentence a story leads with: the one figure that stands out against the period before, in
 * words, or null when nothing does (an ordinary period shows its totals line alone). One function
 * per level (the Overview now; Projects, Sessions, a session and a turn add theirs next to it),
 * sharing what "stands out" means and how a change reads. Pure, tested in headline.test.ts.
 */
import type { Delta, Stats, StatsBucket } from '#lib/api/types.ts';
import type { RangeId } from '#lib/insights/range.ts';
import { duration, integer, money, percent, plural } from '#lib/shared/format.ts';

// ---- thresholds ------------------------------------------------------------------------------

/** A figure stands out once it moved by this share or more, up or down (0.5 is ±50%). */
export const STANDS_OUT = 0.5;

/** Below these (now and before), a big change is noise: $1 to $3 is not news. */
export const MEANINGFUL = {
	/** Dollars. */
	spend: 5,
	/** Seconds of agent working time: an hour. */
	agentTime: 3600,
	/** Seconds of redone agent time: half an hour. */
	redoneTime: 1800,
	commits: 10,
	/** Open possible leaks. */
	leaks: 1
} as const;

/** "Mostly gitprompt" once one project holds this share of the spend. */
export const MOSTLY = 0.5;

// ---- how a change reads ---------------------------------------------------------------------

/** From here up, "tripled" would undersell it: "up 5×". */
const TIMES_FROM = 2.5;
const TRIPLED_FROM = 2;
const DOUBLED_FROM = 1;
/** Down by more than this is no longer "halved": "down 80%". */
const HALVED_TO = -0.67;

/** "doubled", "tripled", "up 60%", "halved", "down 80%". */
export function movement(change: number): string {
	if (change >= TIMES_FROM) return `up ${integer(1 + change)}×`;
	if (change >= TRIPLED_FROM) return 'tripled';
	if (change >= DOUBLED_FROM) return 'doubled';
	if (change > 0) return `up ${percent(change)}`;
	if (change <= -STANDS_OUT && change > HALVED_TO) return 'halved';
	return `down ${percent(-change)}`;
}

const PERIODS: Record<RangeId, string> = {
	'24h': 'today',
	'7d': 'this week',
	'30d': 'this month',
	all: 'so far'
};

/** How the period reads in a sentence: "this week". */
export const periodName = (range: RangeId) => PERIODS[range];

// ---- what stands out ------------------------------------------------------------------------

/** A figure that may lead: its change, the floor that makes it meaningful, and its sentence. */
export type Candidate = {
	delta: Delta | undefined;
	floor: number;
	/** The sentence, given how the figure moved ("doubled") and the period ("this week"). */
	say: (moved: string, period: string) => string;
};

type Standing = { change: number; say: Candidate['say'] };

function standing({ delta, floor, say }: Candidate): Standing | null {
	const change = delta?.change;
	if (change === undefined || Math.abs(change) < STANDS_OUT) return null;
	const size = Math.max(Math.abs(delta?.now ?? 0), Math.abs(delta?.before ?? 0));
	return size >= floor ? { change, say } : null;
}

/** The sentence of the candidate that moved most, or null when none stands out. */
export function standsOut(candidates: Candidate[], range: RangeId): string | null {
	const best = candidates
		.map(standing)
		.filter((s) => s !== null)
		.sort((a, b) => Math.abs(b.change) - Math.abs(a.change))[0];
	return best ? best.say(movement(best.change), periodName(range)) : null;
}

// ---- the Overview ---------------------------------------------------------------------------

/** "278 prompts · 41 sessions · $453 · 181 commits": what any period says, standing out or not. */
export function totalsLine(t: StatsBucket): string {
	return [
		plural(t.prompts, 'prompt'),
		plural(t.sessions, 'session'),
		t.cost_usd > 0 ? money(t.cost_usd) : '',
		t.git_commits > 0 ? plural(t.git_commits, 'commit') : ''
	]
		.filter(Boolean)
		.join(' · ');
}

export type OverviewStory = {
	/** /api/stats with `key_figures` and `previous`. */
	stats: Stats;
	range: RangeId;
	/** The project that spent most, and its spend: "mostly <name>" when it holds most of it. */
	top?: { name: string; cost_usd: number };
};

/** ", mostly gitprompt" when one project holds most of the spend. */
function mostly(stats: Stats, top: OverviewStory['top']): string {
	const total = stats.totals.cost_usd;
	return top && total > 0 && top.cost_usd / total >= MOSTLY ? `, mostly ${top.name}` : '';
}

/** "$38 and 3h 00m of agent time", or the time alone without cost. */
function redoneAmount(stats: Stats): string {
	const r = stats.redone;
	if (!r) return '';
	const time = `${duration(r.agent_time_s)} of agent time`;
	return r.cost_usd === undefined ? time : `${money(r.cost_usd)} and ${time}`;
}

/** The Overview's sentence: spend, agent time, redone work, commits or possible leaks. */
export function overviewHeadline({ stats, range, top }: OverviewStory): string | null {
	const changes = stats.previous?.changes;
	if (!changes) return null;
	return standsOut(
		[
			{
				delta: changes.cost_usd,
				floor: MEANINGFUL.spend,
				say: (moved, period) => `Spend ${moved} ${period}${mostly(stats, top)}`
			},
			{
				delta: changes.agent_time_s,
				floor: MEANINGFUL.agentTime,
				say: (moved, period) =>
					`Agent working time ${moved} ${period}: ${duration(stats.totals.agent_time_s)}`
			},
			{
				delta: changes.redone_agent_time_s,
				floor: MEANINGFUL.redoneTime,
				say: (moved, period) => `Redone work ${moved} ${period}: ${redoneAmount(stats)}`
			},
			{
				delta: changes.git_commits,
				floor: MEANINGFUL.commits,
				say: (moved, period) =>
					`Agent commits ${moved} ${period}: ${integer(stats.totals.git_commits)}`
			},
			{
				delta: changes.possible_leaks,
				floor: MEANINGFUL.leaks,
				say: (moved, period) =>
					`Possible leaks ${moved} ${period}: ${integer(stats.possible_leaks ?? 0)} open`
			}
		],
		range
	);
}
