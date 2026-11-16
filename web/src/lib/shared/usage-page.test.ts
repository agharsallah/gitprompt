import { describe, expect, it, vi } from 'vitest';
import { PageWatch, changedFilters } from '#lib/shared/usage-page.ts';

const url = (path: string) => new URL(path, 'http://127.0.0.1:7878');

describe('PageWatch', () => {
	it('counts a page when its route is opened', () => {
		const watch = new PageWatch();
		expect(watch.visit('/', url('/'))).toEqual(['page.opened.overview']);
		expect(watch.visit('/prompts', url('/prompts?range=7d'))).toEqual(['page.opened.prompts']);
		expect(watch.visit('/sessions/[id]', url('/sessions/abc'))).toEqual(['page.opened.session']);
		expect(watch.visit('/settings', url('/settings'))).toEqual(['page.opened.settings']);
	});

	it('counts a route that is not a page of the app as nothing', () => {
		const watch = new PageWatch();
		expect(watch.visit('/design', url('/design'))).toEqual([]);
		expect(watch.visit(null, url('/nowhere'))).toEqual([]);
	});

	it('counts the filter changed on the page you are on, not the page again', () => {
		const watch = new PageWatch();
		watch.visit('/', url('/?range=7d'));
		expect(watch.visit('/', url('/?range=30d'))).toEqual(['filter.range']);
		expect(watch.visit('/', url('/?range=30d&model=opus'))).toEqual(['filter.model']);
	});

	it('does not count a hash change, paging or a filter that was cleared', () => {
		const watch = new PageWatch();
		watch.visit('/prompts', url('/prompts?model=opus&limit=50'));
		expect(watch.visit('/prompts', url('/prompts?model=opus&limit=50#top'))).toEqual([]);
		expect(watch.visit('/prompts', url('/prompts?model=opus&limit=100&before=9'))).toEqual([]);
		expect(watch.visit('/prompts', url('/prompts'))).toEqual([]);
	});

	it('does not count the filters a link brings to a page it opens', () => {
		const watch = new PageWatch();
		watch.visit('/', url('/'));
		expect(watch.visit('/prompts', url('/prompts?origin=human&starred=true'))).toEqual([
			'page.opened.prompts'
		]);
	});

	it('tells when a new visit starts: on every route change, not on a filter change', () => {
		const onNewPage = vi.fn();
		const watch = new PageWatch(onNewPage);
		watch.visit('/', url('/'));
		watch.visit('/', url('/?range=24h'));
		expect(onNewPage).toHaveBeenCalledTimes(1);
		watch.visit('/prompts', url('/prompts'));
		expect(onNewPage).toHaveBeenCalledTimes(2);
	});
});

describe('changedFilters', () => {
	const kinds = (before: string, after: string) =>
		changedFilters(new URLSearchParams(before), new URLSearchParams(after));

	it('names the kind of each parameter that has a new value', () => {
		expect(kinds('', 'q=retry&tag=x&starred=true&show=failed')).toEqual([
			'search',
			'tag',
			'starred',
			'quick'
		]);
		expect(kinds('', 'source=codex&surface=cli&repo=%2Fapp&outcome=failed')).toEqual([
			'source',
			'surface',
			'repo',
			'outcome'
		]);
	});

	it('compares repeated values as a whole', () => {
		expect(kinds('origin=human', 'origin=human&origin=agent')).toEqual(['origin']);
		expect(kinds('origin=human&origin=agent', 'origin=human&origin=agent')).toEqual([]);
	});

	it('ignores parameters that are not filters', () => {
		expect(kinds('', 'id=4&limit=10&before=3&group_by=day')).toEqual([]);
	});
});
