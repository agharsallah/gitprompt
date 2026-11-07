/**
 * Everything the Overview shows, fetched from the daemon in parallel: each panel's numbers are one
 * /api/stats grouping (or the per-session telemetry) over the same filters, so they always agree
 * with each other and with the Prompts list behind them. No prompt is aggregated in the browser.
 */
import { api } from '#lib/api/client.ts';
import { queryParams, type StatsQuery } from '#lib/api/query.ts';
import type { GroupBy, Prompt, Stats, StatsOrder, Telemetry } from '#lib/api/types.ts';
import { toQuery, without, type InsightFilters } from '#lib/insights/filters.ts';
import { rangeStart } from '#lib/insights/range.ts';

type Fetch = typeof fetch;

export type Overview = {
	filters: InsightFilters;
	query: StatsQuery;
	now: number;
	/** Start of the window; null for all time. */
	start: Date | null;
	/** Clock hours instead of days (the 24-hour range). */
	hourly: boolean;
	/** totals: the headline figures; groups: per day, or per clock hour. */
	activity: Stats;
	week: Stats;
	sessionLengths: Stats;
	/** Every origin, whatever the origin filter: who wrote what. */
	origins: Stats;
	commands: Stats;
	sizes: Stats;
	texts: Stats;
	models: Stats;
	work: Stats;
	querySources: Stats;
	repos: Stats;
	outcomes: Stats;
	toolKinds: Stats;
	tools: Stats;
	telemetry: Telemetry;
	/** The choices of each filter, counted without that filter. */
	options: { source: Stats; surface: Stats; repo: Stats; model: Stats };
	starred: Prompt[];
};

export async function loadOverview(filters: InsightFilters, fetcher: Fetch): Promise<Overview> {
	const now = Date.now();
	const query = toQuery(filters, now);
	const hourly = filters.range.id === '24h';
	const stats = (group_by: GroupBy, extra: StatsQuery = {}, base = query) =>
		api.stats({ ...base, group_by, ...extra }, fetcher);
	const by = (order: StatsOrder, limit: number) => ({ order, limit });

	const results = await all({
		activity: stats(hourly ? 'day_hour' : 'day'),
		week: stats('weekday_hour'),
		sessionLengths: stats('session_length'),
		origins: stats('origin', {}, without(query, 'origin')),
		commands: stats('command', by('prompts', 50)),
		sizes: stats('size'),
		texts: stats('text', by('prompts', 8)),
		models: stats('model'),
		work: stats('work'),
		querySources: stats('query_source', by('cost_usd', 100)),
		repos: stats('repo', by('cost_usd', 50)),
		outcomes: stats('outcome'),
		toolKinds: stats('tool_kind'),
		tools: stats('tool', by('prompts', 500)),
		telemetry: api.telemetry(query, fetcher),
		starred: api.prompts(queryParams({ ...query, starred: 'true', limit: 5 }), fetcher),
		source: stats('source', by('prompts', 50), without(query, 'source')),
		surface: stats('surface', by('prompts', 50), without(query, 'surface')),
		repo: stats('repo', by('prompts', 200), without(query, 'repo')),
		model: stats('model', by('prompts', 50), without(query, 'model'))
	});
	const { source, surface, repo, model, ...panels } = results;
	return {
		filters,
		query,
		now,
		start: rangeStart(filters.range, now),
		hourly,
		...panels,
		options: { source, surface, repo, model }
	};
}

/** Await every promise of a record, keeping its keys. */
async function all<T extends Record<string, Promise<unknown>>>(
	promises: T
): Promise<{ [K in keyof T]: Awaited<T[K]> }> {
	const entries = await Promise.all(
		Object.entries(promises).map(async ([key, p]) => [key, await p] as const)
	);
	return Object.fromEntries(entries) as { [K in keyof T]: Awaited<T[K]> };
}
