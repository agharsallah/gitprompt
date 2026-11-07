import { api } from '#lib/api/client.ts';
import { readFilters } from '#lib/insights/filters.ts';
import { loadOverview } from '#lib/insights/overview.ts';
import { originFilter } from '#lib/prompts/originFilter.svelte.ts';
import { preferences } from '#lib/settings/preferences.ts';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch, url }) => {
	const { default_range, week_start } = await preferences(fetch);
	const filters = readFilters(url.searchParams, default_range, originFilter.origins);
	const [overview, sources] = await Promise.all([loadOverview(filters, fetch), api.sources(fetch)]);
	return { overview, sources, defaultRange: default_range, weekStart: week_start };
};
