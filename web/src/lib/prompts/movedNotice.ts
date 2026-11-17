/**
 * "Search now lives here": the line an old Search bookmark lands on, once per browser. Whether it
 * was seen is kept in localStorage, which can be missing or blocked (private windows): the line
 * then shows on every such visit, never an error. Pure, tested in movedNotice.test.ts.
 */
import { MOVED_PARAM, MOVED_VALUE } from '#lib/prompts/fromSearch.ts';

export const MOVED_KEY = 'gitprompt-search-moved';

type Storage = Pick<globalThis.Storage, 'getItem' | 'setItem'>;

const browserStorage = (): Storage | undefined => {
	try {
		return globalThis.localStorage;
	} catch {
		return undefined;
	}
};

/** The visit came from the old Search page (see `promptsQueryFromSearch`). */
type Params = Pick<URLSearchParams, 'get'>;

export const cameFromSearch = (params: Params): boolean => params.get(MOVED_PARAM) === MOVED_VALUE;

/** The line was already shown, and dismissed or left, in this browser. */
export function wasSeen(storage = browserStorage()): boolean {
	try {
		return storage?.getItem(MOVED_KEY) != null;
	} catch {
		return false;
	}
}

/** Remember the line was seen. */
export function markSeen(storage = browserStorage()): void {
	try {
		storage?.setItem(MOVED_KEY, '1');
	} catch {
		// blocked: it shows again next time
	}
}

/** Whether to show the line: the visit came from Search and the line was not seen before. */
export const shouldShow = (params: Params, storage = browserStorage()): boolean =>
	cameFromSearch(params) && !wasSeen(storage);
