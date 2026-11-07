// The global origin filter: which origins the prompt views show. Remembered per browser; the daemon
// has no default of its own (no `origin` = every origin), so the dashboard always sends this one.
// The Prompts page reads it; a Settings page can show and change it through `originFilter`.
import type { Origin } from '#lib/api/types.ts';
import { ORIGINS } from '#lib/prompts/origin.ts';

/** What you typed, and slash commands you ran (ADR-0010 §5). */
export const DEFAULT_ORIGINS: Origin[] = ['typed', 'slash_command'];
export const ALL_ORIGINS = Object.keys(ORIGINS) as Origin[];

const KEY = 'gitprompt-origins';

/** Known origins only, once each, in ORIGINS order. */
export function parseOrigins(values: Iterable<string>): Origin[] {
	const wanted = new Set(values);
	return ALL_ORIGINS.filter((o) => wanted.has(o));
}

/** No origin selected, or all of them: nothing to filter. */
export const isAll = (origins: Origin[]) =>
	origins.length === 0 || origins.length === ALL_ORIGINS.length;

type Storage = Pick<globalThis.Storage, 'getItem' | 'setItem'>;

// localStorage can be missing or throw (private windows, blocked site data): the choice then lasts
// for this visit only.
const browserStorage = (): Storage | undefined => {
	try {
		return globalThis.localStorage;
	} catch {
		return undefined;
	}
};

export function readOrigins(storage = browserStorage()): Origin[] {
	try {
		const saved = storage?.getItem(KEY);
		if (saved == null) return [...DEFAULT_ORIGINS];
		const values: unknown = JSON.parse(saved);
		return Array.isArray(values) ? parseOrigins(values.map(String)) : [...DEFAULT_ORIGINS];
	} catch {
		return [...DEFAULT_ORIGINS];
	}
}

export function writeOrigins(origins: Origin[], storage = browserStorage()): void {
	try {
		storage?.setItem(KEY, JSON.stringify(parseOrigins(origins)));
	} catch {
		// see browserStorage
	}
}

class OriginFilter {
	/** The remembered choice; an empty list means every origin. */
	origins = $state<Origin[]>(readOrigins());

	set(origins: Origin[]) {
		this.origins = parseOrigins(origins);
		writeOrigins(this.origins);
	}

	reset() {
		this.set(DEFAULT_ORIGINS);
	}
}

export const originFilter = new OriginFilter();
