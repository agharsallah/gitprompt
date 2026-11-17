// The old sidebar's routes all still work: each has a page, or (Search) redirects to the Prompts
// page with the same filters. Pages are found by their file, as SvelteKit does.
import { isRedirect } from '@sveltejs/kit';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { promptsQueryFromSearch } from '#lib/prompts/fromSearch.ts';
import { memoryStorage } from '#lib/testing/storage.ts';

/** Every route that had a link in the sidebar before Search was folded into Prompts. */
const OLD_SIDEBAR = [
	'/',
	'/prompts',
	'/sessions',
	'/projects',
	'/search',
	'/sources',
	'/fields',
	'/settings',
	'/design'
];

const pages = Object.keys(import.meta.glob('/src/routes/**/+page.svelte'));
const hasPage = (route: string) =>
	pages.includes(`/src/routes${route === '/' ? '' : route}/+page.svelte`);

// The daemon is never reached: the default range falls back to its default (7 days).
const offline = () => Promise.reject(new Error('offline'));

type Load = (event: never) => Promise<unknown>;
let load: Load;

/** Where the Search page's load sends this URL, as a path and its query. */
async function redirectOf(search: string) {
	const url = new URL(`http://localhost/search/${search}`);
	try {
		await load({ url, fetch: offline, params: {}, route: { id: '/search' } } as never);
	} catch (error) {
		if (isRedirect(error)) {
			const target = new URL(error.location, url);
			return { status: error.status, path: target.pathname, query: target.searchParams };
		}
		throw error;
	}
	throw new Error('the Search page did not redirect');
}

beforeAll(async () => {
	vi.stubGlobal('localStorage', memoryStorage());
	load = (await import('./+page.ts')).load as Load;
}, 0);
afterEach(() => vi.unstubAllGlobals());

describe('the routes of the old sidebar', () => {
	it.each(OLD_SIDEBAR)('%s still exists', (route) => {
		expect(hasPage(route)).toBe(true);
	});

	it('/search sends you to /prompts', async () => {
		const { status, path } = await redirectOf('');
		expect(status).toBe(307);
		expect(path).toBe('/prompts');
	});
});

describe('an old /search URL on /prompts', () => {
	const urls = [
		'',
		'?q=retry+logic',
		'?q=fix&repo=%2Fwork%2Fapp&tag=keep',
		'?q=fix&range=24h&origin=typed&origin=slash_command',
		'?q=fix&range=all&limit=150',
		'?range=bogus&q=x'
	];

	it.each(urls)('%s keeps its words, project, range, origins and tag', async (search) => {
		const { query } = await redirectOf(search);
		const expected = promptsQueryFromSearch(search, '7d');
		expect([...query]).toEqual([...expected]);
	});

	it('keeps each filter under the name Prompts reads', async () => {
		const { query } = await redirectOf(
			'?q=fix&repo=%2Fa&tag=t&range=30d&origin=typed&origin=subagent'
		);
		expect(query.get('q')).toBe('fix');
		expect(query.get('repo')).toBe('/a');
		expect(query.get('tag')).toBe('t');
		expect(query.get('range')).toBe('30d');
		expect(query.getAll('origin')).toEqual(['typed', 'subagent']);
		expect(query.get('from')).toBe('search');
	});
});
