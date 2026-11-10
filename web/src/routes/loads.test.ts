// The filtering pages' loads read their URL and nothing else. SvelteKit runs a load when a link is
// only hovered (data-sveltekit-preload-data in app.html): a load that kept or restored a view
// would change what you see without a click. That was the bug: hovering "24h" made Overview
// reopen at 24 hours.
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

// Any page load: called below with the parts of its event that these loads read.
type Load = (event: never) => unknown;

function memoryStorage() {
	const items = new Map<string, string>();
	const writes: string[] = [];
	return {
		writes,
		getItem: (k: string) => items.get(k) ?? null,
		setItem: (k: string, v: string) => {
			writes.push(k);
			items.set(k, v);
		},
		removeItem: (k: string) => {
			writes.push(k);
			items.delete(k);
		}
	};
}

const PAGES: [string, () => Promise<{ load: Load }>][] = [
	['/', () => import('./+page.ts')],
	['/prompts/', () => import('./prompts/+page.ts')],
	['/sessions/', () => import('./sessions/+page.ts')],
	['/projects/', () => import('./projects/+page.ts')],
	['/search/', () => import('./search/+page.ts')]
];

/** The range a page's load data shows. */
const rangeOf = (data: { range?: { id: string }; filters?: { range?: { id: string } } }) =>
	data.range?.id ?? data.filters?.range?.id;

let storage: ReturnType<typeof memoryStorage>;
// Importing a page compiles its whole module graph: once, before the tests time anything.
beforeAll(async () => {
	vi.stubGlobal('localStorage', memoryStorage());
	await Promise.all(PAGES.map(([, module]) => module()));
}, 120_000);
beforeEach(() => {
	storage = memoryStorage();
	vi.stubGlobal('localStorage', storage);
});
afterEach(() => vi.unstubAllGlobals());

// The daemon is never reached: preferences fall back to their defaults, the lists stay pending.
const fetch = () => Promise.reject(new Error('offline'));

describe('page loads', () => {
	it.each(PAGES)('%s shows the range its URL names, and stores nothing', async (path, module) => {
		const { load } = await module();
		const url = new URL(`http://localhost${path}?range=24h&model=m`);
		const data = (await load({ url, fetch, params: {}, route: { id: path } } as never)) as object;
		expect(rangeOf(data)).toBe('24h');
		expect(storage.writes).toEqual([]);
	});

	it.each(PAGES)(
		'%s without a range shows its default, whatever was kept',
		async (path, module) => {
			storage.setItem('gitprompt-view-overview', 'range=30d');
			storage.setItem('gitprompt-view-prompts', 'range=30d');
			const { load } = await module();
			const url = new URL(`http://localhost${path}`);
			const data = (await load({ url, fetch, params: {}, route: { id: path } } as never)) as object;
			expect(rangeOf(data)).toBe(path === '/prompts/' ? 'all' : '7d');
		}
	);
});
