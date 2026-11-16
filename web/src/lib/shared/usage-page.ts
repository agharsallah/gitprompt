/**
 * What a visit to a page counts, worked out from where the page is: the root layout hands every
 * URL change to a `PageWatch`, so no page counts itself. Opening a page (a new route) counts the
 * page; changing a filter on the page you are on (the URL's query) counts the kind of filter.
 */
import { ROUTE_IDS } from '#lib/shared/links.ts';
import {
	PAGES,
	type FilterKind,
	type PageName,
	type UsageAction
} from '#lib/shared/usage-actions.ts';

/** The pages, by route id (links.ts). Others (the design reference) are not counted. */
const PAGE_OF_ROUTE = new Map<string, PageName>(PAGES.map((name) => [ROUTE_IDS[name], name]));

/** The query parameter of each kind of filter. */
const FILTER_OF_PARAM: Record<string, FilterKind> = {
	range: 'range',
	origin: 'origin',
	source: 'source',
	surface: 'surface',
	repo: 'repo',
	model: 'model',
	outcome: 'outcome',
	q: 'search',
	tag: 'tag',
	starred: 'starred',
	show: 'quick'
};

/** The kinds of filter whose value in `next` differs from `previous` (a removed one is not a use). */
export function changedFilters(previous: URLSearchParams, next: URLSearchParams): FilterKind[] {
	const kinds = new Set<FilterKind>();
	for (const [param, kind] of Object.entries(FILTER_OF_PARAM)) {
		const now = next.getAll(param).join('\n');
		if (now && now !== previous.getAll(param).join('\n')) kinds.add(kind);
	}
	return [...kinds];
}

export class PageWatch {
	#route: string | null = null;
	#params = new URLSearchParams();
	#onNewPage: () => void;

	/** `onNewPage` runs whenever the route changes (a new visit starts: see `forgetSeen`). */
	constructor(onNewPage: () => void = () => {}) {
		this.#onNewPage = onNewPage;
	}

	/** What the move to this route and URL counts: the page when the route is new, else the filters. */
	visit(route: string | null, url: { search: string }): UsageAction[] {
		const previous = { route: this.#route, params: this.#params };
		this.#route = route;
		this.#params = new URLSearchParams(url.search);
		if (route !== previous.route) {
			this.#onNewPage();
			const page = route === null ? undefined : PAGE_OF_ROUTE.get(route);
			return page ? [`page.opened.${page}`] : [];
		}
		return changedFilters(previous.params, this.#params).map((kind) => `filter.${kind}` as const);
	}
}
