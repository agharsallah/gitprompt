// What a turn produced, as the reader's "What came of it" says it: one sentence (files written,
// lines added and removed, commits after, cost), the first files and how many more, and the commits
// with their rows on the project's Commits tab. Pure, tested in cameOfIt.test.ts.
import type { Commit, PromptDetail } from '#lib/api/types.ts';
import { parseDiffstat, type Diffstat } from '#lib/prompts/diffstat.ts';
import { factsView } from '#lib/prompts/facts.ts';
import { integer, plural, relativePath } from '#lib/shared/format.ts';
import { projectHref } from '#lib/shared/links.ts';
import { projectId } from '#lib/stories/story.ts';

/** The parts of a prompt this reads: the files it wrote, the commits after it, its cost. */
export type TurnResult = Pick<PromptDetail, 'summary' | 'facts' | 'context' | 'commits'>;

export type CameOfIt = {
	sentence: string;
	/** The first files written, relative to the repository. */
	files: string[];
	/** Files written past `files`: "and 7 more". */
	more: number;
	commits: { commit: Commit; href: string | undefined }[];
	/** Nothing was written or committed: the sentence says so and nothing follows. */
	empty: boolean;
};

/** How many files the reader lists before "and N more". */
export const SHOWN_FILES = 5;

export const NOTHING = 'Nothing was written or committed';

export function cameOfIt(turn: TurnResult): CameOfIt {
	const written = turn.summary.files_written ?? [];
	const commits = turn.commits.commits ?? [];
	const root = turn.context.git_root;
	const parts = [
		filesPart(written.length, parseDiffstat(turn.commits.diffstat)),
		commits.length ? plural(commits.length, 'commit') : 'no commit',
		factsView(turn.facts).cost
	];
	const empty = !written.length && !commits.length;
	return {
		sentence: empty ? NOTHING : parts.filter(Boolean).join(', '),
		files: written.slice(0, SHOWN_FILES).map((file) => relativePath(file, root)),
		more: Math.max(0, written.length - SHOWN_FILES),
		commits: commits.map((commit) => ({ commit, href: commitRowHref(root, commit.hash) })),
		empty
	};
}

/**
 * "Wrote 6 files (+212 −40)": the lines are the commits' range, so a turn that wrote nothing but
 * committed says what the commits changed instead.
 */
function filesPart(written: number, stat: Diffstat | null): string | null {
	const lines = stat ? ` (+${integer(stat.insertions)} −${integer(stat.deletions)})` : '';
	if (written) return `Wrote ${plural(written, 'file')}${lines}`;
	return stat ? `Changed ${plural(stat.files, 'file')}${lines}` : null;
}

/** A commit's row on its project's Commits tab; nothing outside a repository. */
export function commitRowHref(root: string | undefined, hash: string): string | undefined {
	return root
		? projectHref(projectId(root), { tab: 'commits', anchor: `commit-${hash}` })
		: undefined;
}
