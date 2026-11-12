/**
 * The time range and each filtering page's filters, kept in this browser so the sidebar can take
 * you back to them. Four rules, nothing else:
 *
 * 1. The URL is the state. A page's load reads its URL and nothing else, so a link preloaded on
 *    hover, a reload or a shared link all show what the URL says.
 * 2. Whenever a page's URL changes, it is kept (`keepView`, called from the root layout only): its
 *    `range` as the one time range of every page (pick 24 hours anywhere, every page opens at 24
 *    hours), the rest of its query as that page's own filters.
 * 3. The sidebar links to each page with the range and its filters (`viewSearch`): hovering a
 *    link shows where it goes.
 * 4. Opening GitPrompt (a new tab, the desktop app) at a page with no query shows its view
 *    (`startView`). Every other link, such as an Overview square, shows exactly what it says.
 *
 * Kept: the query without what belongs to one visit (the selected prompt, how many rows are
 * loaded) and without empty values. localStorage can be missing or blocked (private windows):
 * nothing is remembered then.
 */
import { FACETS } from '#lib/insights/filters.ts';
import { RANGES } from '#lib/insights/range.ts';
import { pinLabel } from '#lib/prompts/pinned.ts';
import { SORTS } from '#lib/sessions/load.ts';
import { folder } from '#lib/shared/format.ts';

type Storage = Pick<globalThis.Storage, 'getItem' | 'setItem' | 'removeItem'>;

/** The pages that keep a view: route id -> its storage key and name. */
export const VIEWS = {
	'/': { key: 'gitprompt-view-overview', label: 'Overview' },
	'/prompts': { key: 'gitprompt-view-prompts', label: 'Prompts' },
	'/sessions': { key: 'gitprompt-view-sessions', label: 'Sessions' },
	'/projects': { key: 'gitprompt-view-projects', label: 'Projects' },
	'/search': { key: 'gitprompt-view-search', label: 'Search' }
} as const;
export type ViewPage = keyof typeof VIEWS;
export const VIEW_PAGES = Object.keys(VIEWS) as ViewPage[];

/** The time range, shared by every page: a range id. */
export const RANGE_KEY = 'gitprompt-view-range';

/** `off`: remember nothing (Settings → General). */
export const REMEMBER_KEY = 'gitprompt-remember-filters';

/** Keys of the previous version (one shared range, merged per parameter): see `dropOldKeys`. */
export const OLD_KEYS = [
	'gitprompt-range',
	'gitprompt-overview-filters',
	'gitprompt-sessions-view'
];

/** Parameters of one visit, not of the view: the selected prompt, how many rows are loaded. */
const TRANSIENT = new Set(['id', 'limit']);

export const isViewPage = (route: string | null | undefined): route is ViewPage =>
	!!route && Object.hasOwn(VIEWS, route);

const browserStorage = (): Storage | undefined => {
	try {
		return globalThis.localStorage;
	} catch {
		return undefined;
	}
};

function get(storage: Storage | undefined, key: string): string | null {
	try {
		return storage?.getItem(key) ?? null;
	} catch {
		return null;
	}
}

function put(storage: Storage | undefined, key: string, value: string | null): void {
	try {
		if (value == null) storage?.removeItem(key);
		else storage?.setItem(key, value);
	} catch {
		// blocked: this visit only
	}
}

/** The part of a query that is kept: `?id=4&range=30d&model=&limit=80` -> `range=30d`. */
export function viewQuery(search: string | URLSearchParams): string {
	const kept = new URLSearchParams();
	for (const [key, value] of new URLSearchParams(search)) {
		if (value && !TRANSIENT.has(key)) kept.append(key, value);
	}
	return kept.toString();
}

export function isRemembering(storage = browserStorage()): boolean {
	return get(storage, REMEMBER_KEY) !== 'off';
}

/** Turn remembering on or off; either way, start from nothing kept. */
export function setRemembering(on: boolean, storage = browserStorage()): void {
	forgetViews(storage);
	put(storage, REMEMBER_KEY, on ? null : 'off');
}

const isRange = (id: string | null): id is string => RANGES.some((r) => r.id === id);

/** A query split into its time range (a known range id, or null) and the page's own filters. */
function split(search: string): { range: string | null; filters: string } {
	const params = new URLSearchParams(viewQuery(search));
	const range = params.get('range');
	params.delete('range');
	return { range: isRange(range) ? range : null, filters: params.toString() };
}

/** Store `value` under `key` (`''` removes it), writing only when it changed. */
function write(storage: Storage | undefined, key: string, value: string): void {
	if (value !== (get(storage, key) ?? '')) put(storage, key, value || null);
}

/** The kept time range, a range id (`''` when none). */
export function savedRange(storage = browserStorage()): string {
	const range = get(storage, RANGE_KEY);
	return isRemembering(storage) && isRange(range) ? range : '';
}

/** A page's kept filters, as a query without the range (`''` when none). */
export function savedView(page: ViewPage, storage = browserStorage()): string {
	return isRemembering(storage) ? split(get(storage, VIEWS[page].key) ?? '').filters : '';
}

/** Keep a page's URL: its range for every page (when it names one), the rest as its filters. */
export function keepView(page: ViewPage, search: string, storage = browserStorage()): void {
	if (!isRemembering(storage)) return;
	const { range, filters } = split(search);
	if (range) write(storage, RANGE_KEY, range);
	write(storage, VIEWS[page].key, filters);
}

/** What the sidebar links to: `?` with the range and the page's filters, or `''`. */
export function viewSearch(page: ViewPage, storage = browserStorage()): string {
	const range = savedRange(storage);
	const params = new URLSearchParams(savedView(page, storage));
	const query = new URLSearchParams([...(range ? [['range', range]] : []), ...params]).toString();
	return query ? `?${query}` : '';
}

/**
 * Where to go instead when GitPrompt opens at `route` + `search`: the page's view when the URL has
 * no query at all and the page has a view, else null (stay). A link to one prompt (`?id=`) stays.
 */
export function startView(
	route: string | null,
	search: string,
	storage = browserStorage()
): string | null {
	if (!isViewPage(route) || new URLSearchParams(search).size) return null;
	return viewSearch(route, storage) || null;
}

export function forgetView(page: ViewPage, storage = browserStorage()): void {
	put(storage, VIEWS[page].key, null);
}

export function forgetViews(storage = browserStorage()): void {
	for (const page of VIEW_PAGES) forgetView(page, storage);
	forgetRange(storage);
	dropOldKeys(storage);
}

/** Remove what the previous version kept (run once when the app starts). */
export function dropOldKeys(storage = browserStorage()): void {
	for (const key of OLD_KEYS) if (get(storage, key) != null) put(storage, key, null);
}

/** Forget the time range (a new default range: every page opens with it). */
export function forgetRange(storage = browserStorage()): void {
	put(storage, RANGE_KEY, null);
}

/** One kept value in words, named the way its control names it. */
function named(key: string, value: string): string {
	if (key === 'sort') return `Sort: ${SORTS[value as keyof typeof SORTS]?.label ?? value}`;
	if (key === 'project') return `Project: ${folder(value) ?? value}`;
	if (key === 'q') return `Search: “${value}”`;
	if (key === 'tag') return `Tag: ${value}`;
	// Own keys only: a remembered URL may carry any name, `toString` included.
	if (Object.hasOwn(FACETS, key)) {
		const facet = FACETS[key as keyof typeof FACETS];
		return `${facet.label}: ${facet.name(value)}`;
	}
	return pinLabel(key, value) ?? `${key}: ${value}`;
}

/** A page's kept filters in words, for the Settings page. */
export function describeView(page: ViewPage, storage = browserStorage()): string {
	const parts = [...new URLSearchParams(savedView(page, storage))].map(([k, v]) => named(k, v));
	return parts.length ? parts.join(', ') : 'Nothing remembered';
}

/** The kept time range in words, for the Settings page. */
export function describeRange(storage = browserStorage()): string {
	return RANGES.find((r) => r.id === savedRange(storage))?.label ?? 'Nothing remembered';
}
