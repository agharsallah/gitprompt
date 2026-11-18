/**
 * The Projects page's rows, from /api/stats grouped by project (`repo`: the git root, or the
 * working directory outside a repository) and split by day, tool kind, branch and origin. Pure, tested in
 * summary.test.ts.
 */
import type { StatsBucket } from '#lib/api/types.ts';
import type { Slot } from '#lib/insights/series.ts';
import { asKind, TOOL_KINDS, type ToolKind } from '#lib/insights/tools.ts';
import { ORIGINS } from '#lib/prompts/origin.ts';
import { parseOrigins } from '#lib/prompts/originFilter.svelte.ts';
import { folder } from '#lib/shared/format.ts';

export type ProjectSummary = {
	/** The `repo` filter value. */
	key: string;
	name: string;
	/** owner/name of the origin remote, when there is one. */
	repo: string | null;
	/** Sent from inside a git repository (a branch was seen). */
	git: boolean;
	/** Every prompt the origin filter keeps. */
	prompts: number;
	/** Of those, the ones you typed (and slash commands): what the project page counts. */
	typed: number;
	/** Of those, the ones the tool or other agents sent: the project page counts them apart. */
	fromAgents: number;
	sessions: number;
	cost: number;
	lastAt: string;
	branches: string[];
	/** Files each turn wrote, summed over turns. */
	filesWritten: number;
	failed: number;
	kinds: { kind: ToolKind; count: number }[];
	/** Prompts per slot of the window. */
	daily: number[];
};

/** The projects of the window, grouped by `repo` and split four ways (the groups of /api/stats). */
export type ProjectSplits = {
	byDay: StatsBucket[];
	byKind: StatsBucket[];
	byBranch: StatsBucket[];
	byOrigin: StatsBucket[];
};

const parts = (groups: StatsBucket[], key: string | null) =>
	groups.find((g) => g.key === key)?.parts ?? [];

/** A known origin the tool or another agent sends (not typed by you). */
const isAgentOrigin = (key: string | null) =>
	parseOrigins(key ? [key] : []).some((origin) => ORIGINS[origin].system);

/** How many of a project's prompts agents sent; 0 when the origins were not split. */
const sentByAgents = (byOrigin: StatsBucket[], key: string | null) =>
	parts(byOrigin, key)
		.filter((p) => isAgentOrigin(p.key))
		.reduce((sum, p) => sum + p.prompts, 0);

/**
 * One row per project of `byDay` (its order), with the kinds, branches and origins of the other
 * groupings and a daily series over `slots`.
 */
export function projectSummaries(
	{ byDay, byKind, byBranch, byOrigin }: ProjectSplits,
	slots: Slot[]
): ProjectSummary[] {
	return byDay.map((g) => {
		const fromAgents = sentByAgents(byOrigin, g.key);
		const days = new Map(parts(byDay, g.key).map((p) => [p.key, p.prompts]));
		const kinds = new Map<ToolKind, number>();
		for (const p of parts(byKind, g.key))
			kinds.set(asKind(p.key), (kinds.get(asKind(p.key)) ?? 0) + p.tool_calls);
		const branches = parts(byBranch, g.key)
			.map((p) => p.key)
			.filter((b): b is string => !!b);
		return {
			key: g.key ?? '',
			name: g.detail ?? folder(g.key) ?? 'No folder',
			repo: g.detail,
			git: branches.length > 0,
			prompts: g.prompts,
			typed: g.prompts - fromAgents,
			fromAgents,
			sessions: g.sessions,
			cost: g.cost_usd,
			lastAt: g.last_at ?? '',
			branches,
			filesWritten: g.files_written,
			failed: g.failed,
			kinds: TOOL_KINDS.filter((k) => kinds.get(k)).map((kind) => ({
				kind,
				count: kinds.get(kind)!
			})),
			daily: slots.map((s) => days.get(s.key) ?? 0)
		};
	});
}
