// The kept range and filters (lib/settings/views.ts) as reactive state: the sidebar's links follow
// them, and Settings shows, forgets and switches them off. All logic is in views.ts; this only re-reads.
import {
	dropOldKeys,
	forgetRange,
	forgetView,
	isRemembering,
	keepView,
	setRemembering,
	viewSearch,
	VIEW_PAGES,
	type ViewPage
} from '#lib/settings/views.ts';

const read = () =>
	Object.fromEntries(VIEW_PAGES.map((p) => [p, viewSearch(p)])) as Record<ViewPage, string>;

class Views {
	on = $state(isRemembering());
	/** What the sidebar links to for each page: `?query` or `''`. */
	search = $state(read());

	constructor() {
		dropOldKeys();
	}

	/** Re-read every link (the range is in all of them), changing only those that differ. */
	#refresh() {
		for (const [page, search] of Object.entries(read()) as [ViewPage, string][]) {
			if (search !== this.search[page]) this.search[page] = search;
		}
	}

	/** Keep the URL you are on (the root layout calls this on every URL change). */
	keep(page: ViewPage, url: { search: string }) {
		keepView(page, url.search);
		this.#refresh();
	}

	set(on: boolean) {
		setRemembering(on);
		this.on = on;
		this.#refresh();
	}

	forget(page: ViewPage) {
		forgetView(page);
		this.#refresh();
	}

	/** A new default range: every page opens with it. */
	forgetRange() {
		forgetRange();
		this.#refresh();
	}
}

export const views = new Views();
