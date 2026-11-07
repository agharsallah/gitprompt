import { api } from '#lib/api/client.ts';
import { readFilters, toQuery } from '#lib/insights/filters.ts';
import { originFilter } from '#lib/prompts/originFilter.svelte.ts';
import { preferences } from '#lib/settings/preferences.ts';
import type { PageLoad } from './$types';

// Every project of the window, from the daemon's statistics (no cap): one grouping by project,
// split by day (the sparkline), tool kind and branch. Origins: the remembered choice.
export const load: PageLoad = async ({ fetch, url }) => {
	const { default_range } = await preferences(fetch);
	const filters = readFilters(url.searchParams, default_range, originFilter.origins);
	const now = Date.now();
	const query = { ...toQuery(filters, now), group_by: 'repo' as const, limit: 500 };
	const [byDay, byKind, byBranch] = await Promise.all([
		api.stats({ ...query, split_by: 'day' }, fetch),
		api.stats({ ...query, split_by: 'tool_kind' }, fetch),
		api.stats({ ...query, split_by: 'branch' }, fetch)
	]);
	return { filters, defaultRange: default_range, now, byDay, byKind, byBranch };
};
