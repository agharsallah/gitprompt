import { describe, expect, it } from 'vitest';
import {
	describeRange,
	describeView,
	dropOldKeys,
	forgetRange,
	forgetViews,
	isRemembering,
	keepView,
	OLD_KEYS,
	RANGE_KEY,
	REMEMBER_KEY,
	savedRange,
	savedView,
	setRemembering,
	startView,
	VIEW_PAGES,
	VIEWS,
	viewQuery,
	viewSearch
} from '#lib/settings/views.ts';
import { memoryStorage } from '#lib/testing/storage.ts';

const blocked = {
	getItem: () => {
		throw new Error('blocked');
	},
	setItem: () => {
		throw new Error('blocked');
	},
	removeItem: () => {
		throw new Error('blocked');
	}
};

describe('what a view keeps', () => {
	it('keeps the query without the selected prompt, the page size and empty values', () => {
		expect(viewQuery('?id=4&range=30d&model=&limit=80&source=codex')).toBe(
			'range=30d&source=codex'
		);
	});

	it('keeps repeated values (two outcomes from an Overview link)', () => {
		expect(viewQuery('outcome=failed&outcome=interrupted')).toBe(
			'outcome=failed&outcome=interrupted'
		);
	});

	it('keeps nothing of a query that only selects a prompt', () => {
		expect(viewQuery('?id=12')).toBe('');
	});
});

describe('one time range for every page', () => {
	it('the reported case: 24 hours picked on Overview, every page opens at 24 hours', () => {
		const s = memoryStorage();
		keepView('/', '?range=24h', s);
		expect(VIEW_PAGES.map((p) => viewSearch(p, s))).toEqual(Array(5).fill('?range=24h'));
	});

	it('the range last picked on any page wins', () => {
		const s = memoryStorage();
		keepView('/', '?range=24h', s);
		keepView('/prompts', '?range=all', s);
		expect([viewSearch('/', s), savedRange(s)]).toEqual(['?range=all', 'all']);
	});

	it('a page opened without a range (a link to a session’s prompts) keeps the range', () => {
		const s = memoryStorage();
		keepView('/', '?range=30d', s);
		keepView('/prompts', '?session=s1', s);
		expect(viewSearch('/prompts', s)).toBe('?range=30d&session=s1');
	});

	it('an unknown range is not kept', () => {
		const s = memoryStorage();
		keepView('/', '?range=30d', s);
		keepView('/', '?range=forever', s);
		s.items.set(RANGE_KEY, 'forever');
		expect(viewSearch('/', s)).toBe('');
	});
});

describe('each page keeps its own filters', () => {
	it('a page does not change another page’s filters', () => {
		const s = memoryStorage();
		keepView('/', '?range=30d&model=m', s);
		keepView('/sessions', '?range=24h&sort=longest', s);
		keepView('/projects', '?range=all', s);
		expect(VIEW_PAGES.map((p) => savedView(p, s))).toEqual(['model=m', '', 'sort=longest', '', '']);
		expect(viewSearch('/', s)).toBe('?range=all&model=m');
	});

	it('the last query wins as a whole: a filter left out is no longer kept', () => {
		const s = memoryStorage();
		keepView('/', '?range=30d&model=m', s);
		keepView('/', '?range=30d', s);
		expect(viewSearch('/', s)).toBe('?range=30d');
		expect(s.items.has(VIEWS['/'].key)).toBe(false);
	});

	it('writes only when something changed (live refreshes and prompt selection cost nothing)', () => {
		const s = memoryStorage();
		keepView('/prompts', '?range=7d&model=m', s);
		const after = s.writes();
		for (let id = 0; id < 20; id++) keepView('/prompts', `?range=7d&model=m&id=${id}`, s);
		expect(s.writes()).toBe(after);
	});

	it('ignores broken or blocked storage', () => {
		expect(() => keepView('/', '?range=7d', blocked)).not.toThrow();
		expect(viewSearch('/', blocked)).toBe('');
		const s = memoryStorage();
		s.items.set(VIEWS['/'].key, 'id=3&range=30d&&model=');
		expect(viewSearch('/', s)).toBe('');
	});
});

describe('opening GitPrompt', () => {
	it('a page opened with no query shows its view', () => {
		const s = memoryStorage();
		keepView('/', '?range=30d', s);
		expect(startView('/', '', s)).toBe('?range=30d');
	});

	it('a link with a query shows what it says, even one that only opens a prompt', () => {
		const s = memoryStorage();
		keepView('/prompts', '?range=all&model=m', s);
		expect(startView('/prompts', '?range=7d&weekday=2', s)).toBeNull();
		expect(startView('/prompts', '?id=3', s)).toBeNull();
	});

	it('stays on a page without a view, or one that keeps none', () => {
		const s = memoryStorage();
		expect(startView('/', '', s)).toBeNull();
		keepView('/', '?range=30d', s);
		expect(startView('/settings', '', s)).toBeNull();
		expect(startView('/sessions/[id]', '', s)).toBeNull();
		expect(startView(null, '', s)).toBeNull();
	});
});

describe('turning it off and forgetting', () => {
	it('when off, nothing is kept or restored, and what was kept is forgotten', () => {
		const s = memoryStorage();
		keepView('/', '?range=30d', s);
		setRemembering(false, s);
		expect(isRemembering(s)).toBe(false);
		keepView('/', '?range=24h', s);
		expect([viewSearch('/', s), startView('/', '', s)]).toEqual(['', null]);
		expect([...s.items.keys()]).toEqual([REMEMBER_KEY]);
		setRemembering(true, s);
		expect([isRemembering(s), s.items.size]).toEqual([true, 0]);
	});

	it('a new default range forgets the range, and only the range', () => {
		const s = memoryStorage();
		keepView('/', '?range=30d&model=m', s);
		forgetRange(s);
		expect([viewSearch('/', s), viewSearch('/projects', s)]).toEqual(['?model=m', '']);
	});

	it('forgets every view and what the previous version kept', () => {
		const s = memoryStorage();
		for (const key of OLD_KEYS) s.items.set(key, '{"range":"24h"}');
		keepView('/', '?range=30d', s);
		forgetViews(s);
		expect(s.items.size).toBe(0);
	});

	it('drops the previous version’s keys, writing nothing when there are none', () => {
		const s = memoryStorage();
		s.items.set(OLD_KEYS[0], '{"range":"24h"}');
		dropOldKeys(s);
		dropOldKeys(s);
		expect([s.items.size, s.writes()]).toEqual([0, 1]);
	});
});

describe('in words, for Settings', () => {
	it('names each value the way its control does', () => {
		const s = memoryStorage();
		expect([describeView('/', s), describeRange(s)]).toEqual([
			'Nothing remembered',
			'Nothing remembered'
		]);
		keepView('/', '?range=30d&model=claude-opus-5-5&repo=/w/app', s);
		keepView('/sessions', '?sort=longest&project=/w/api&q=fix', s);
		keepView('/prompts', '?range=24h&weekday=2&has=commit&tag=wip', s);
		expect(describeRange(s)).toBe('24 hours');
		expect(describeView('/', s)).toBe('Model: Opus 5.5, Project: app');
		expect(describeView('/sessions', s)).toBe('Sort: Most turns, Project: api, Search: “fix”');
		expect(describeView('/prompts', s)).toBe('Tuesdays, Agent committed, Tag: wip');
	});

	it('names an unknown key plainly, even one Object itself has (a link may carry any name)', () => {
		const s = memoryStorage();
		keepView('/prompts', '?toString=1&constructor=x&hasOwnProperty=y&zzz=2', s);
		expect(describeView('/prompts', s)).toBe(
			'toString: 1, constructor: x, hasOwnProperty: y, zzz: 2'
		);
	});

	it('gives every page its own key under the gitprompt- prefix', () => {
		const keys = [REMEMBER_KEY, RANGE_KEY, ...OLD_KEYS, ...VIEW_PAGES.map((p) => VIEWS[p].key)];
		expect(new Set(keys).size).toBe(keys.length);
		expect(keys.every((k) => k.startsWith('gitprompt-'))).toBe(true);
	});
});
