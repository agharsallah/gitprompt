/**
 * Filters and time range remembered in this browser, so a page opened from the sidebar (or the
 * desktop app's start) shows the view you left instead of the defaults.
 *
 * The URL stays the state: a page's load passes its query through `rememberView` first. For each
 * remembered parameter, a value in the URL (even empty: an "every …" choice) wins and is stored; a
 * missing one is filled in from what was stored. So controls must set a parameter explicitly, never
 * delete it to mean "the default". The time range is shared by every insight page; other filters
 * belong to one page (a scope).
 *
 * Cost: a few small localStorage reads per navigation, and a write only when a value changed.
 * localStorage can be missing or blocked (private windows): nothing is remembered then.
 */
import { FACETS } from '#lib/insights/filters.ts';
import { RANGES } from '#lib/insights/range.ts';
import { SORTS } from '#lib/sessions/load.ts';
import { folder } from '#lib/shared/format.ts';

type Storage = Pick<globalThis.Storage, 'getItem' | 'setItem' | 'removeItem'>;

/** `off`: remember nothing (Settings → This browser). */
export const REMEMBER_KEY = 'gitprompt-remember-filters';

export const SCOPES = {
	range: { key: 'gitprompt-range', params: ['range'], label: 'Time range' },
	overview: {
		key: 'gitprompt-overview-filters',
		params: ['source', 'surface', 'repo', 'model'],
		label: 'Overview filters'
	},
	sessions: { key: 'gitprompt-sessions-view', params: ['project', 'sort'], label: 'Sessions view' }
} as const;
export type Scope = keyof typeof SCOPES;
type Saved = Record<string, string>;

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

export function isRemembering(storage = browserStorage()): boolean {
	return get(storage, REMEMBER_KEY) !== 'off';
}

/** Turn remembering on or off; either way, start from nothing stored. */
export function setRemembering(on: boolean, storage = browserStorage()): void {
	forgetAll(storage);
	put(storage, REMEMBER_KEY, on ? null : 'off');
}

export function forget(scope: Scope, storage = browserStorage()): void {
	put(storage, SCOPES[scope].key, null);
}

export function forgetAll(storage = browserStorage()): void {
	for (const scope of Object.keys(SCOPES) as Scope[]) forget(scope, storage);
}

/** What a scope keeps: its own parameters with a non-empty string value only. */
export function saved(scope: Scope, storage = browserStorage()): Saved {
	const text = get(storage, SCOPES[scope].key);
	if (!text) return {};
	try {
		const value: unknown = JSON.parse(text);
		if (typeof value !== 'object' || value === null) return {};
		const known: readonly string[] = SCOPES[scope].params;
		return Object.fromEntries(
			Object.entries(value).filter(([k, v]) => known.includes(k) && typeof v === 'string' && v)
		) as Saved;
	} catch {
		return {};
	}
}

/**
 * The page's query with the remembered values filled in for the parameters it lacks; the values it
 * has are stored. Parameters outside `scopes` are left as they are.
 */
export function rememberView(
	params: URLSearchParams,
	scopes: Scope[],
	storage = browserStorage()
): URLSearchParams {
	if (!isRemembering(storage)) return params;
	const out = new URLSearchParams(params);
	for (const scope of scopes) {
		const before = saved(scope, storage);
		const after: Saved = {};
		for (const param of SCOPES[scope].params) {
			const value = params.has(param) ? params.get(param) : before[param];
			if (value) after[param] = value;
			if (!params.has(param) && value) out.set(param, value);
		}
		const text = Object.keys(after).length ? JSON.stringify(after) : null;
		if (text !== (Object.keys(before).length ? JSON.stringify(before) : null)) {
			put(storage, SCOPES[scope].key, text);
		}
	}
	return out;
}

/** One remembered value in words, named the way its filter names it. */
function named(param: string, value: string): string {
	if (param === 'range') return RANGES.find((r) => r.id === value)?.label ?? value;
	if (param === 'sort') return `Sort: ${SORTS[value as keyof typeof SORTS]?.label ?? value}`;
	if (param === 'project') return `Project: ${folder(value) ?? value}`;
	const facet = FACETS[param as keyof typeof FACETS];
	return facet ? `${facet.label}: ${facet.name(value)}` : `${param}: ${value}`;
}

/** The remembered values of a scope in words, for the Settings page. */
export function describe(scope: Scope, storage = browserStorage()): string {
	const parts = Object.entries(saved(scope, storage)).map(([k, v]) => named(k, v));
	return parts.length ? parts.join(', ') : 'Nothing remembered';
}
