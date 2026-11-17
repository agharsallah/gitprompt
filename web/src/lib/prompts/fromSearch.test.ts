import { describe, expect, it } from 'vitest';
import { carriedFilters, promptsQueryFromSearch } from '#lib/prompts/fromSearch.ts';

/** The filters of a query, without the marker of the visit, as a sorted list to compare. */
const filtersOf = (query: URLSearchParams) =>
	[...query].filter(([key]) => key !== 'from').map(([key, value]) => `${key}=${value}`);

describe('an old /search URL on the Prompts page', () => {
	// Each row: the old URL's query, the setting `default_range`, and the Prompts filters it means.
	const table: [name: string, old: string, defaultRange: '7d' | '24h', expected: string[]][] = [
		['nothing at all', '', '7d', ['range=7d']],
		['nothing, with another default range', '', '24h', ['range=24h']],
		['words', '?q=retry+logic', '7d', ['range=7d', 'q=retry logic']],
		['words with spaces around', '?q=%20retry%20', '7d', ['range=7d', 'q=retry']],
		['empty words', '?q=', '7d', ['range=7d']],
		['a project', '?q=a&repo=%2Fwork%2Fapp', '7d', ['range=7d', 'q=a', 'repo=/work/app']],
		['a tag', '?q=a&tag=keep', '7d', ['range=7d', 'q=a', 'tag=keep']],
		['a range', '?q=a&range=30d', '7d', ['range=30d', 'q=a']],
		['an unknown range', '?q=a&range=1y', '24h', ['range=24h', 'q=a']],
		[
			'origins',
			'?q=a&origin=typed&origin=slash_command',
			'7d',
			['range=7d', 'q=a', 'origin=typed', 'origin=slash_command']
		],
		['more rows', '?q=a&limit=100', '7d', ['range=7d', 'q=a', 'limit=100']],
		['the first page size is no news', '?q=a&limit=50', '7d', ['range=7d', 'q=a']],
		['too many rows', '?q=a&limit=9999', '7d', ['range=7d', 'q=a', 'limit=200']],
		[
			'everything at once',
			'?q=fix&repo=%2Fa&tag=t&range=all&origin=typed&limit=150',
			'7d',
			['range=all', 'q=fix', 'repo=/a', 'tag=t', 'origin=typed', 'limit=150']
		],
		['what Search never read', '?q=a&model=m&zzz=1', '7d', ['range=7d', 'q=a']]
	];

	it.each(table)('%s', (_name, old, defaultRange, expected) => {
		expect(filtersOf(promptsQueryFromSearch(old, defaultRange))).toEqual(expected);
	});

	it('marks the visit, so Prompts can say where Search went', () => {
		expect(promptsQueryFromSearch('?q=a', '7d').get('from')).toBe('search');
	});

	it('accepts ready-made params as well as text', () => {
		const params = new URLSearchParams([['q', 'a']]);
		expect(filtersOf(promptsQueryFromSearch(params, '7d'))).toEqual(['range=7d', 'q=a']);
	});
});

describe('carriedFilters', () => {
	it('is what Prompts reads under the same names, and no range', () => {
		expect([...carriedFilters('range=30d&q=a&repo=r&tag=t&origin=typed')]).toEqual([
			['q', 'a'],
			['repo', 'r'],
			['tag', 't'],
			['origin', 'typed']
		]);
	});
});
