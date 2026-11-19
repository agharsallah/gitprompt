/**
 * The weekly recap as a card on the Overview, for when no desktop app shows it as a notification:
 * shown the first time the Overview opens in the week it was sent, until dismissed. What was
 * shown is kept in this browser by the recap's week (its `due` instant), so the next week's recap
 * shows again. localStorage can be missing or blocked (private windows): the card then shows on
 * every visit until dismissed.
 */
import type { Recap } from '#lib/api/types.ts';

type Storage = Pick<globalThis.Storage, 'getItem' | 'setItem'>;

/** The week (`due`) of the recap last shown here. */
export const RECAP_SEEN_KEY = 'gitprompt-recap-seen';

const browserStorage = (): Storage | undefined => {
	try {
		return globalThis.localStorage;
	} catch {
		return undefined;
	}
};

function seenWeek(storage: Storage | undefined): string | null {
	try {
		return storage?.getItem(RECAP_SEEN_KEY) ?? null;
	} catch {
		return null;
	}
}

function markSeen(recap: Recap, storage: Storage | undefined) {
	try {
		storage?.setItem(RECAP_SEEN_KEY, recap.due);
	} catch {
		// Blocked storage: nothing is remembered.
	}
}

/**
 * Whether to show `recap` on this visit: one was sent this week and this browser has not shown
 * it yet. Showing it counts: the next visit in the same week does not show it again.
 */
export function firstShowing(
	recap: Recap | null | undefined,
	storage: Storage | undefined = browserStorage()
): recap is Recap {
	if (!recap || seenWeek(storage) === recap.due) return false;
	markSeen(recap, storage);
	return true;
}

/** Dismiss `recap`: its week is kept as seen, so it stays hidden until next week's. */
export function dismissRecap(recap: Recap, storage: Storage | undefined = browserStorage()) {
	markSeen(recap, storage);
}
