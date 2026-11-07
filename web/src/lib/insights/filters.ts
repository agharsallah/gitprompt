/**
 * The Overview's filters, kept in its URL so a view can be linked: time range (`range`), origins
 * (`origin`, repeated), source, project (`repo`) and model. Turned into the daemon's query, and into
 * links to the Prompts page with the same filters (every chart is a doorway). Pure, tested in
 * filters.test.ts.
 */
import { queryParams, type StatsQuery } from '#lib/api/query.ts';
import type { Origin, PromptQuery } from '#lib/api/types.ts';
import { ALL_ORIGINS, isAll, parseOrigins } from '#lib/prompts/originFilter.svelte.ts';
import { parseRange, rangeStart, type Range, type RangeId } from '#lib/insights/range.ts';

export type InsightFilters = {
	range: Range;
	/** The origins shown; empty = every origin. */
	origins: Origin[];
	source?: string;
	repo?: string;
	model?: string;
};

/** The single-valued filters the bar offers besides range and origin, in the order shown. */
export const DIMENSIONS = ['source', 'repo', 'model'] as const;
export type Dimension = (typeof DIMENSIONS)[number];

/**
 * Filters from the URL. Without `?range=`, the `default_range` setting; without `?origin=`, the
 * origins remembered in this browser (default: what you typed and slash commands).
 */
export function readFilters(
	params: URLSearchParams,
	defaultRange: RangeId,
	remembered: Origin[]
): InsightFilters {
	const inUrl = params.getAll('origin');
	const filters: InsightFilters = {
		range: parseRange(params.get('range'), defaultRange),
		origins: inUrl.length ? parseOrigins(inUrl) : remembered
	};
	for (const key of DIMENSIONS) {
		const value = params.get(key);
		if (value) filters[key] = value;
	}
	return filters;
}

/** The daemon query for these filters (`now` fixes the start of the window). */
export function toQuery(filters: InsightFilters, now: number): StatsQuery {
	const start = rangeStart(filters.range, now);
	return {
		since: start?.toISOString(),
		origin: isAll(filters.origins) ? [] : filters.origins,
		source: filters.source,
		repo: filters.repo,
		model: filters.model ? [filters.model] : []
	};
}

/** The same filters without one of them: to count what each choice of that filter would keep. */
export function without(query: StatsQuery, key: Dimension | 'origin'): StatsQuery {
	const { [key]: _, ...rest } = query;
	return rest;
}

/**
 * A link to the Prompts page that lists the prompts behind a number: these filters plus `extra`
 * (a day, an outcome, a tool...). `extra` wins over the range (e.g. a day's own since/until).
 */
export function promptsHref(base: string, query: StatsQuery, extra: PromptQuery = {}): string {
	// The Prompts page reads no origin as "the remembered choice": name every origin instead.
	const origin = query.origin?.length ? query.origin : ALL_ORIGINS;
	const search = queryParams({ ...query, origin, ...extra }).toString();
	return `${base}${search ? `?${search}` : ''}`;
}

/** A link to the prompts behind a number, under the page's filters (see promptsHref). */
export type Doorway = (extra?: PromptQuery) => string;
