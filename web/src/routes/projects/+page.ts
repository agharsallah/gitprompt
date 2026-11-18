import { api } from '#lib/api/client.ts';
import { defer } from '#lib/async/resource.svelte.ts';
import { readFilters, toQuery } from '#lib/insights/filters.ts';
import { originFilter } from '#lib/prompts/originFilter.svelte.ts';
import { preferences } from '#lib/settings/preferences.ts';
import type { PageLoad } from './$types';

// Every project of the window, from the daemon's statistics (no cap): one grouping by project,
// split by day (the sparkline), tool kind, branch and origin (typed vs. sent by agents). Origins: the remembered choice. The
// statistics stream in.
export const load: PageLoad = async ({ fetch, url }) => {
	const { default_range } = await preferences(fetch);
	const filters = readFilters(url.searchParams, default_range, originFilter.origins);
	const now = Date.now();
	const query = { ...toQuery(filters, now), group_by: 'repo' as const, limit: 500 };
	const stats = Promise.all([
		api.stats({ ...query, split_by: 'day' }, fetch),
		api.stats({ ...query, split_by: 'tool_kind' }, fetch),
		api.stats({ ...query, split_by: 'branch' }, fetch),
		api.stats({ ...query, split_by: 'origin' }, fetch)
	]).then(([byDay, byKind, byBranch, byOrigin]) => ({ byDay, byKind, byBranch, byOrigin }));
	return { filters, now, stats: defer(stats) };
};
