/**
 * The Overview's filters, kept in its URL so a view can be linked: time range (`range`), origins
 * (`origin`, repeated), source, app (`surface`), project (`repo`) and model. Turned into the daemon's query, and into
 * links to the Prompts page with the same filters (every chart is a doorway). Pure, tested in
 * filters.test.ts.
 */
import { queryParams, type StatsQuery } from '#lib/api/query.ts';
import type { Origin, PromptQuery, Stats, TurnOutcome } from '#lib/api/types.ts';
import { TURN_OUTCOMES } from '#lib/insights/outcome.ts';
import { folder, modelName } from '#lib/shared/format.ts';
import { ALL_ORIGINS, isAll, parseOrigins } from '#lib/prompts/originFilter.svelte.ts';
import { parseRange, rangeStart, type Range, type RangeId } from '#lib/insights/range.ts';

export type InsightFilters = {
	range: Range;
	/** The origins shown; empty = every origin. */
	origins: Origin[];
	source?: string;
	/** Where prompts were typed (a Surface id). */
	surface?: string;
	repo?: string;
	model?: string;
};

/** The single-valued filters the bar offers besides range and origin, in the order shown. */
export const DIMENSIONS = ['source', 'surface', 'repo', 'model'] as const;
export type Dimension = (typeof DIMENSIONS)[number];

/**
 * Every filter offered as a select, on the Overview and the Prompts page (FacetSelects.svelte): its
 * label, its "any" choice, and how one of its values is named (a group's `detail` from /api/stats,
 * e.g. a tool's name or a remote's owner/name, wins).
 */
export const FACETS = {
	source: { label: 'Tool', any: 'Every tool', name: (key: string) => key },
	// Where the prompt was typed: the daemon names each app (`detail`); prompts whose tool doesn't
	// say have no surface and are only counted under "Every app".
	surface: { label: 'App', any: 'Every app', name: (key: string) => key },
	repo: { label: 'Project', any: 'Every project', name: (key: string) => folder(key) ?? key },
	model: { label: 'Model', any: 'Every model', name: modelName },
	outcome: {
		label: 'Outcome',
		any: 'Every outcome',
		name: (key: string) => TURN_OUTCOMES[key as TurnOutcome]?.label ?? key
	}
} as const;
export type Facet = keyof typeof FACETS;

/**
 * The choices of a facet select: each value the other filters keep, named and counted. A chosen
 * value that none of them has (filtered away, or from a link) stays first, so the select shows it.
 */
export function facetChoices(facet: Facet, stats: Stats | undefined, chosen?: string) {
	const name = (key: string, detail: string | null) => detail ?? FACETS[facet].name(key);
	const list = (stats?.groups ?? [])
		.filter((g) => g.key)
		.map((g) => ({ value: g.key!, label: `${name(g.key!, g.detail)} (${g.prompts})` }));
	return chosen && !list.some((o) => o.value === chosen)
		? [{ value: chosen, label: name(chosen, null) }, ...list]
		: list;
}

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
		surface: filters.surface ? [filters.surface] : [],
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
 * A link to the Prompts page that lists the prompts behind a number: the same range (as `range`,
 * so the Prompts page's range picker shows it) and filters, plus `extra` (a day, an outcome, a
 * tool...). A day's own `since`/`until` narrow the range there.
 */
export function promptsHref(
	base: string,
	filters: InsightFilters,
	extra: PromptQuery = {}
): string {
	const { since: _, ...query } = toQuery(filters, Date.now());
	// The Prompts page reads no origin as "the remembered choice": name every origin instead.
	const origin = query.origin?.length ? query.origin : ALL_ORIGINS;
	const params = queryParams({ ...query, origin, ...extra });
	return `${base}?${new URLSearchParams([['range', filters.range.id], ...params])}`;
}

/** A link to the prompts behind a number, under the page's filters (see promptsHref). */
export type Doorway = (extra?: PromptQuery) => string;
