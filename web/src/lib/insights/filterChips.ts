/**
 * The folded filter bar's chips: each filter that is on (besides range and origin) as a named,
 * removable chip beside the "Filters · N" button, and the link that clears them all. Derived from
 * the same state as the selects (filters.ts activeFilters), so the button never hides a filter.
 * Pure, tested in filterChips.test.ts.
 */
import type { Stats } from '#lib/api/types.ts';
import {
	activeFilters,
	DIMENSIONS,
	dimensionParams,
	FACETS,
	type Dimension,
	type InsightFilters
} from '#lib/insights/filters.ts';
import { withQuery } from '#lib/shared/links.ts';

/** Where the bar is: the page's path and its query (a URL, or SvelteKit's `page.url`). */
export type Here = { pathname: string; search: string };

export type FilterChip = {
	dimension: Dimension;
	/** "Project: app". */
	label: string;
	/** This page without that filter (everything else in its URL kept). */
	href: string;
};

/** This page's URL with the dimensions set to those of `filters`; the rest of the URL kept. */
function hrefWith(url: Here, filters: InsightFilters): string {
	const params = new URLSearchParams(url.search);
	for (const d of DIMENSIONS) params.delete(d);
	for (const [d, value] of dimensionParams(filters)) params.append(d, value);
	return withQuery(url.pathname, params);
}

/** A filter value by the name the daemon gives it (`detail`), else as its facet names it. */
function valueName(dimension: Dimension, value: string, options?: Stats): string {
	const detail = options?.groups.find((g) => g.key === value)?.detail;
	return detail ?? FACETS[dimension].name(value);
}

/** One chip per filter that is on, in the bar's order. */
export function filterChips(
	filters: InsightFilters,
	options: Partial<Record<Dimension, Stats>>,
	url: Here
): FilterChip[] {
	return activeFilters(filters).map(({ dimension, value }) => ({
		dimension,
		label: `${FACETS[dimension].label}: ${valueName(dimension, value, options[dimension])}`,
		href: hrefWith(url, { ...filters, [dimension]: undefined })
	}));
}

/** This page without any of the chips' filters: range and origins stay, as they were. */
export const clearedHref = (url: Here, filters: InsightFilters) =>
	hrefWith(url, { range: filters.range, origins: filters.origins });

/** The button's label: "Filters", or "Filters · 2" while two are on. */
export const foldLabel = (count: number) => (count ? `Filters · ${count}` : 'Filters');
