/**
 * The Projects page's rows, from /api/stats grouped by project (`repo`: the git root, or the
 * working directory outside a repository) and split by day, tool kind and branch. Pure, tested in
 * summary.test.ts.
 */
import type { StatsBucket } from '#lib/api/types.ts';
import type { Slot } from '#lib/insights/series.ts';
import { asKind, TOOL_KINDS, type ToolKind } from '#lib/insights/tools.ts';
import { folder } from '#lib/shared/format.ts';

export type ProjectSummary = {
	/** The `repo` filter value. */
	key: string;
	name: string;
	/** owner/name of the origin remote, when there is one. */
	repo: string | null;
	/** Sent from inside a git repository (a branch was seen). */
	git: boolean;
	prompts: number;
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

const parts = (groups: StatsBucket[], key: string | null) =>
	groups.find((g) => g.key === key)?.parts ?? [];

/**
 * One row per project of `byDay` (its order), with the kinds and branches of the other two
 * groupings and a daily series over `slots`.
 */
export function projectSummaries(
	byDay: StatsBucket[],
	byKind: StatsBucket[],
	byBranch: StatsBucket[],
	slots: Slot[]
): ProjectSummary[] {
	return byDay.map((g) => {
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
