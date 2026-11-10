import { api } from '#lib/api/client.ts';
import { forget, read } from '#lib/settings/browser.ts';
import {
	DISMISS_KEY,
	dismissValue,
	isDismissed,
	pendingUpdates,
	type PendingUpdate
} from '#lib/sources/updates.ts';

/**
 * Connections that need an update, shared by the sidebar's hint and its Connectors dot. Fetched
 * when the sidebar asks (on navigation and when the window regains focus): an update only appears
 * after a GitPrompt upgrade or a spec change, so there is nothing to poll for.
 */
class Updates {
	list = $state<PendingUpdate[]>([]);
	#stored = $state<string | null>(read(DISMISS_KEY));
	dismissed = $derived(isDismissed(this.list, this.#stored));

	refresh() {
		api
			.sources()
			.then((sources) => (this.list = pendingUpdates(sources, Date.now())))
			// No hint is better than a wrong one: the Connectors page reports the error.
			.catch(() => {});
	}

	/** "Later": hide the hint until another connection needs an update. The dot stays. */
	dismiss() {
		const value = dismissValue(this.list);
		try {
			localStorage.setItem(DISMISS_KEY, value);
		} catch {
			// storage blocked: hidden for this visit only
		}
		this.#stored = value;
	}

	/** Settings' reset: show the hint again. */
	reset() {
		forget(DISMISS_KEY);
		this.#stored = null;
	}
}

export const updates = new Updates();
