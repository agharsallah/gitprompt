import { api } from '#lib/api/client.ts';
import { defer } from '#lib/async/resource.svelte.ts';
import { readFilters, toQuery, without } from '#lib/insights/filters.ts';
import { originFilter } from '#lib/prompts/originFilter.svelte.ts';
import { preferences } from '#lib/settings/preferences.ts';
import { rememberView } from '#lib/settings/remembered.ts';
import type { PageLoad } from './$types';

/** Results shown at first, and added by "Show more". */
const PAGE = 50;

// URL: ?q= (every word, in the typed text), ?repo= (a project), ?tag=, ?range=, ?origin= (repeated;
// without it, the origins remembered in this browser), ?limit=. The form shows at once; the
// results, counts and projects stream in.
export const load: PageLoad = async ({ fetch, url }) => {
	const { default_range } = await preferences(fetch);
	const params = rememberView(url.searchParams, ['range']);
	const filters = readFilters(params, default_range, originFilter.origins);
	const q = params.get('q')?.trim() ?? '';
	const tag = params.get('tag')?.trim() || undefined;
	const limit = Math.min(200, Math.max(PAGE, Number(params.get('limit')) || PAGE));
	const base = { ...toQuery(filters, Date.now()), tag };
	const results = q ? api.search({ ...base, q, limit }, fetch) : Promise.resolve(null);
	const facets = Promise.all([
		api.stats({ ...without(base, 'origin'), q, group_by: 'origin' }, fetch),
		api.stats({ ...without(base, 'repo'), group_by: 'repo', limit: 200 }, fetch)
	]).then(([byOrigin, projects]) => ({
		originCounts: Object.fromEntries(byOrigin.groups.map((g) => [g.key, g.prompts])),
		projects: projects.groups.filter((g) => g.key)
	}));
	return {
		q,
		tag,
		limit,
		page: PAGE,
		filters,
		results: defer(results),
		facets: defer(facets)
	};
};
