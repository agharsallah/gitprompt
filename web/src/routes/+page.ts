import { api } from '#lib/api/client.ts';
import { defer } from '#lib/async/resource.svelte.ts';
import { readFilters } from '#lib/insights/filters.ts';
import { loadOverview } from '#lib/insights/overview.ts';
import { originFilter } from '#lib/prompts/originFilter.svelte.ts';
import { preferences } from '#lib/settings/preferences.ts';
import type { PageLoad } from './$types';

// The panels' data streams in (lib/async/resource.svelte.ts): the page and its skeleton show at
// once. Preferences are cached after the first page and never fail.
export const load: PageLoad = async ({ fetch, url }) => {
	const { default_range, week_start } = await preferences(fetch);
	const filters = readFilters(url.searchParams, default_range, originFilter.origins);
	const content = Promise.all([loadOverview(filters, fetch), api.sources(fetch)]).then(
		([overview, sources]) => ({ overview, sources })
	);
	return { filters, content: defer(content), weekStart: week_start };
};
