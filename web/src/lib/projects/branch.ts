/**
 * The prompts behind a branch, as the daemon renders them for a pull request
 * (GET /api/export/branch: Markdown, secrets masked, paths relative): its URL, which branch the
 * Commits tab means, and copying the block. Tested in branch.test.ts.
 */
import type { StoryEntry } from '#lib/api/types.ts';
import { fetchPreview, type Preview } from '#lib/export/preview.ts';
import { copyCounted } from '#lib/shared/clipboard.ts';
import { toasts } from '#lib/ui/toast.svelte.ts';

export const branchUrl = (project: string, branch: string) =>
	`/api/export/branch?${new URLSearchParams({ project, branch })}`;

/** The branch of the latest prompt you typed in the project that names one; none in a folder. */
export function currentBranch(story: {
	phases: { items: Pick<StoryEntry, 'branch'>[] }[];
}): string | undefined {
	const entries = story.phases.flatMap((phase) => phase.items);
	return entries.findLast((entry) => entry.branch)?.branch ?? undefined;
}

/** The block as the daemon renders it: what the preview shows and the copy writes. */
export const loadBranch = (
	project: string,
	branch: string,
	fetcher: typeof fetch = fetch
): Promise<Preview> => fetchPreview(branchUrl(project, branch), fetcher);

/** What copying needs from the page; the defaults are the real ones, tests pass their own. */
export type CopyDeps = {
	copy: typeof copyCounted;
	say: (message: string, tone: 'info' | 'error') => void;
	fetcher: typeof fetch;
};

const real: CopyDeps = {
	copy: copyCounted,
	say: (message, tone) => toasts.show(message, { tone }),
	fetcher: (...args) => fetch(...args)
};

/**
 * Copy the block, counted as `copy.branch`, and say so (or that the copy failed). Call it in the
 * click itself: the write starts at once and is handed the fetch (clipboard.ts).
 */
export async function copyBranch(
	project: string,
	branch: string,
	deps: CopyDeps = real
): Promise<boolean> {
	const text = loadBranch(project, branch, deps.fetcher).then((preview) => preview.text);
	const ok = await deps.copy(text, 'copy.branch');
	if (ok) deps.say(`Copied the prompts behind ${branch}`, 'info');
	else deps.say('Could not copy the prompts behind the branch', 'error');
	return ok;
}
