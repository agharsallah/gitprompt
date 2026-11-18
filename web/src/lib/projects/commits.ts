/**
 * The Commits tab of a project, as pure functions: which commits it shows, how it names a group,
 * what it says when there is nothing to show, and how the keyboard moves between rows.
 */
import type {
	CommitGroup,
	CommitGrouping,
	CommitTrace,
	ProjectCommits,
	TraceCounts
} from '#lib/api/types.ts';
import { dayLabel, excerpt, integer, plural } from '#lib/shared/format.ts';
import type { PanelState } from '#lib/ui/panelState.ts';
import { READY } from '#lib/ui/panelState.ts';

/** A commit's row is `commit-<hash>`: a link (from the palette) lands on it. */
export const COMMIT_ANCHOR = 'commit-';
export const commitAnchor = (hash: string) => `${COMMIT_ANCHOR}${hash}`;

/** Which commits the table shows: all, those that followed a prompt, or those made by hand. */
export type CommitShow = 'all' | 'prompt' | 'hand';

export const fromPrompt = (trace: Pick<CommitTrace, 'how'>) => trace.how !== 'by_hand';

const keeps: Record<CommitShow, (trace: CommitTrace) => boolean> = {
	all: () => true,
	prompt: fromPrompt,
	hand: (trace) => !fromPrompt(trace)
};

/** The groups with only the commits `show` keeps, counted again; a group left empty goes. */
export function showing(groups: CommitGroup[], show: CommitShow): CommitGroup[] {
	if (show === 'all') return groups;
	return groups
		.map((group) => {
			const commits = group.commits.filter(keeps[show]);
			return { ...group, commits, count: commits.length };
		})
		.filter((group) => group.count > 0);
}

/** The opening sentence, part by part: each part is what one filter keeps. */
export function summaryParts(counts: TraceCounts): { show: CommitShow; text: string }[] {
	return [
		{ show: 'all', text: plural(counts.total, 'commit') },
		{ show: 'prompt', text: `${integer(counts.traced + counts.likely)} followed a prompt` },
		{ show: 'hand', text: `${integer(counts.by_hand)} by hand` }
	];
}

/** A group's heading: the phase, the branch or the day; commits without one were made by hand. */
export function groupTitle(key: string | null, by: CommitGrouping, now: Date): string {
	if (key === null) return 'Made by hand';
	if (by === 'phase') return `Phase ${key}`;
	return by === 'day' ? dayLabel(key, now) : key;
}

/** The first line of a prompt, on one line: what a commit row shows under the commit. */
export const firstLine = (text: string) => excerpt(text.trim().split('\n')[0], 120);

/** What the table says in place of rows: not a repository, no commits, or nothing that matches. */
export function commitsState(data: ProjectCommits, shown: number, q: string): PanelState {
	if (!data.git) {
		return {
			kind: 'empty',
			title: 'Not a git repository',
			message:
				'No branches and no commits to show. Run git init, or open a folder that is a repository; prompts and cost still tell the story.'
		};
	}
	if (!data.counts.total) {
		return {
			kind: 'empty',
			title: 'No commits yet',
			message: 'Commits made in this repository since your first prompt here show up here.'
		};
	}
	if (shown) return READY;
	const words = q.trim();
	return {
		kind: 'empty',
		title: words ? `No commit matches “${words}”` : 'No commit of this kind',
		message: 'Search by subject, file or hash, or show every commit.'
	};
}

/** The row ↑ or ↓ moves to from `at` (none yet: the first or the last), or none for other keys. */
export function rowStep(at: number | null, key: string, rows: number): number | undefined {
	if (!rows) return undefined;
	if (key === 'ArrowDown') return at === null ? 0 : Math.min(rows - 1, at + 1);
	if (key === 'ArrowUp') return at === null ? rows - 1 : Math.max(0, at - 1);
	return undefined;
}

/** What a key does in the table: move to a row, open the row's prompt, or its commit on the remote. */
export type RowAction = { kind: 'move'; to: number } | { kind: 'prompt' } | { kind: 'remote' };

type RowKey = Pick<KeyboardEvent, 'key' | 'metaKey' | 'ctrlKey'>;

/** How a row opens what it links to: a page on the forge (in a new tab), or a prompt. */
export type RowOpener = { url: (href: string) => void; prompt: (id: number) => void };

/**
 * Open a row's prompt, or its commit on the forge (`trace.url`, from the daemon); nothing when the
 * row has no such link (a commit made by hand, a repository without a known remote).
 */
export function openRow(
	kind: Exclude<RowAction['kind'], 'move'>,
	trace: CommitTrace,
	open: RowOpener
): void {
	if (kind === 'remote') {
		if (trace.url) open.url(trace.url);
	} else if (trace.prompt) open.prompt(trace.prompt.id);
}

/**
 * ↑ ↓ move between rows; ⌘↵ (Ctrl+↵ off a Mac) opens the commit on the remote; ↵ opens the prompt
 * when the row itself has focus (on a link inside it, ↵ follows that link).
 */
export function rowAction(
	event: RowKey,
	at: number | null,
	rows: number,
	onRow: boolean
): RowAction | undefined {
	if (event.key === 'Enter') {
		if (event.metaKey || event.ctrlKey) return { kind: 'remote' };
		return onRow ? { kind: 'prompt' } : undefined;
	}
	const to = rowStep(at, event.key, rows);
	return to === undefined ? undefined : { kind: 'move', to };
}
