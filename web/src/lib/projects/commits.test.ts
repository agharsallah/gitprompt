import { describe, expect, it } from 'vitest';
import {
	commitAnchor,
	commitsState,
	firstLine,
	groupTitle,
	openRow,
	rowAction,
	showing,
	summaryParts
} from '#lib/projects/commits.ts';
import { group, projectCommits, trace } from '#lib/testing/commits.ts';

const NOW = new Date('2026-11-17T12:00:00+01:00');

describe('which commits the table shows', () => {
	const traced = trace('traced');
	const likely = trace('likely');
	const hand = trace('by_hand');
	const groups = [
		group('2', [traced, hand]),
		group(null, [trace('by_hand')]),
		group('1', [likely])
	];

	it('shows every commit by default, groups untouched', () => {
		expect(showing(groups, 'all')).toBe(groups);
	});

	it('“followed a prompt” keeps traced and likely commits, counts again and drops emptied groups', () => {
		expect(showing(groups, 'prompt')).toEqual([group('2', [traced]), group('1', [likely])]);
	});

	it('“by hand” keeps only the commits no prompt came before', () => {
		const kept = showing(groups, 'hand');
		expect(kept.map((g) => [g.key, g.count])).toEqual([
			['2', 1],
			[null, 1]
		]);
		expect(kept[0].commits).toEqual([hand]);
	});
});

describe('the opening sentence', () => {
	it('says how many commits, how many followed a prompt (likely included) and how many by hand', () => {
		const parts = summaryParts({ total: 212, traced: 180, likely: 7, by_hand: 25 });
		expect(parts).toEqual([
			{ show: 'all', text: '212 commits' },
			{ show: 'prompt', text: '187 followed a prompt' },
			{ show: 'hand', text: '25 by hand' }
		]);
	});

	it('speaks of one commit in the singular', () => {
		expect(summaryParts({ total: 1, traced: 1, likely: 0, by_hand: 0 })[0].text).toBe('1 commit');
	});
});

describe('group headings', () => {
	it('name the phase, the branch or the day', () => {
		expect(groupTitle('3', 'phase', NOW)).toBe('Phase 3');
		expect(groupTitle('feat/tabs', 'branch', NOW)).toBe('feat/tabs');
		expect(groupTitle('2026-11-17', 'day', NOW)).toBe('Today');
		expect(groupTitle('2026-11-05', 'day', NOW)).toBe('Thursday 5 November');
	});

	it('call the commits without a phase or a branch made by hand', () => {
		expect(groupTitle(null, 'phase', NOW)).toBe('Made by hand');
		expect(groupTitle(null, 'branch', NOW)).toBe('Made by hand');
	});
});

describe('what the table says instead of rows', () => {
	it('says a folder is not a git repository before anything else', () => {
		const state = commitsState(projectCommits({ git: false, groups: [] }), 0, '');
		expect(state).toMatchObject({ kind: 'empty', title: 'Not a git repository' });
	});

	it('says there are no commits yet in a repository without any', () => {
		const state = commitsState(projectCommits({ groups: [] }), 0, '');
		expect(state).toMatchObject({ kind: 'empty', title: 'No commits yet' });
	});

	it('names the words a search found nothing for', () => {
		const data = projectCommits({ matched: 0, groups: [] });
		data.counts.total = 4;
		expect(commitsState(data, 0, ' uploader ')).toMatchObject({
			title: 'No commit matches “uploader”'
		});
		expect(commitsState(data, 0, '')).toMatchObject({ title: 'No commit of this kind' });
	});

	it('draws the rows when there are some', () => {
		expect(commitsState(projectCommits(), 1, 'x')).toEqual({ kind: 'ready' });
	});
});

describe('the keyboard in the table', () => {
	const key = (k: string, mod: Partial<KeyboardEvent> = {}) => ({
		key: k,
		metaKey: false,
		ctrlKey: false,
		...mod
	});

	it('moves down and up between rows, stopping at the ends, from outside to the first or last', () => {
		expect(rowAction(key('ArrowDown'), null, 3, false)).toEqual({ kind: 'move', to: 0 });
		expect(rowAction(key('ArrowUp'), null, 3, false)).toEqual({ kind: 'move', to: 2 });
		expect(rowAction(key('ArrowDown'), 1, 3, true)).toEqual({ kind: 'move', to: 2 });
		expect(rowAction(key('ArrowDown'), 2, 3, true)).toEqual({ kind: 'move', to: 2 });
		expect(rowAction(key('ArrowUp'), 0, 3, true)).toEqual({ kind: 'move', to: 0 });
		expect(rowAction(key('ArrowDown'), null, 0, false)).toBeUndefined();
	});

	it('opens the prompt with ↵ on a row, and leaves ↵ on a link inside it to the link', () => {
		expect(rowAction(key('Enter'), 0, 3, true)).toEqual({ kind: 'prompt' });
		expect(rowAction(key('Enter'), 0, 3, false)).toBeUndefined();
	});

	it('opens the commit on the remote with ⌘↵ or Ctrl+↵', () => {
		expect(rowAction(key('Enter', { metaKey: true }), 0, 3, false)).toEqual({ kind: 'remote' });
		expect(rowAction(key('Enter', { ctrlKey: true }), 0, 3, true)).toEqual({ kind: 'remote' });
	});

	it('ignores other keys', () => {
		expect(rowAction(key('a'), 0, 3, true)).toBeUndefined();
	});
});

describe('opening a row', () => {
	/** What `openRow` asked to open, recorded instead of opened. */
	function opened(kind: 'prompt' | 'remote', row: ReturnType<typeof trace>) {
		const asked: string[] = [];
		openRow(kind, row, {
			url: (href) => asked.push(href),
			prompt: (id) => asked.push(`prompt ${id}`)
		});
		return asked;
	}

	it('opens the commit on its forge with ⌘↵ when the daemon gives its url', () => {
		const url = 'https://github.com/acme/app/commit/c0ffee';
		expect(opened('remote', trace('traced', { url }))).toEqual([url]);
	});

	it('opens nothing with ⌘↵ without a url', () => {
		expect(opened('remote', trace('traced'))).toEqual([]);
	});

	it('opens the prompt with ↵, and nothing for a commit made by hand', () => {
		const traced = trace('traced');
		expect(opened('prompt', traced)).toEqual([`prompt ${traced.prompt?.id}`]);
		expect(opened('prompt', trace('by_hand'))).toEqual([]);
	});
});

describe('rows', () => {
	it('are anchored by hash', () => {
		expect(commitAnchor('a9f0971')).toBe('commit-a9f0971');
	});

	it('show the first line of the prompt, on one line', () => {
		expect(firstLine('  Port the store\nto rusqlite  ')).toBe('Port the store');
	});
});
