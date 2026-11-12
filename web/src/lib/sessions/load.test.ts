import { describe, expect, it } from 'vitest';
import { isSort, promptsOf, SORTS } from '#lib/sessions/load.ts';
import { fakeFetch } from '#lib/testing/fetch.ts';
import { prompt } from '#lib/testing/builders.ts';

/** A page of `size` prompts with ids counting down from `from`. */
const page = (from: number, size: number) =>
	Array.from({ length: size }, (_, i) => prompt({ id: from - i }));

describe('sorts', () => {
	it('accepts only the sorts it offers', () => {
		expect(Object.keys(SORTS).every(isSort)).toBe(true);
		expect([isSort('cheapest'), isSort(null), isSort('')]).toEqual([false, false, false]);
	});
});

describe('promptsOf', () => {
	it('asks nothing for no session', async () => {
		const { fetch, calls } = fakeFetch();
		expect(await promptsOf([], {}, fetch)).toEqual([]);
		expect(calls).toEqual([]);
	});

	it('pages through a long session, older pages before the last id', async () => {
		const pages = [page(1000, 200), page(800, 200), page(600, 3)];
		const { fetch, calls } = fakeFetch(() => ({ body: pages.shift() }));
		const prompts = await promptsOf(['s1', 's2'], { source: 'codex' }, fetch);
		expect(prompts).toHaveLength(403);
		const asked = calls.map((c) => new URL(c.url, 'http://x').searchParams);
		expect(asked.map((p) => p.getAll('session'))).toEqual([
			['s1', 's2'],
			['s1', 's2'],
			['s1', 's2']
		]);
		expect(asked.map((p) => p.get('before'))).toEqual([null, '801', '601']);
		expect(asked.every((p) => p.get('limit') === '200' && p.get('source') === 'codex')).toBe(true);
	});

	it('stops after a bounded number of pages', async () => {
		const { fetch, calls } = fakeFetch(() => ({ body: page(1_000_000, 200) }));
		expect(await promptsOf(['s'], {}, fetch)).toHaveLength(20 * 200);
		expect(calls).toHaveLength(20);
	});
});
