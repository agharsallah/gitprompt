import { api } from '#lib/api/client.ts';
import { queryParams } from '#lib/api/query.ts';
import { defer } from '#lib/async/resource.svelte.ts';
import { PROMPT_FACETS } from '#lib/prompts/PromptFilters.svelte';
import {
	FILTER_KEYS,
	type Filters,
	type Has,
	type Origin,
	type PromptQuery,
	type Stats,
	type TurnOutcome
} from '#lib/api/types.ts';
import type { Facet } from '#lib/insights/filters.ts';
import { originFilter, parseOrigins } from '#lib/prompts/originFilter.svelte.ts';
import { MAX_LIMIT, PAGE } from '#lib/prompts/paging.ts';
import type { PageLoad } from './$types';

// Only the server-side filters (and the page size) are read here, so SvelteKit reruns this load
// when one of them changes, and not when a prompt is selected (?id=) or a quick filter (?show=)
// is toggled: those are handled in the page component.
//
// Model, outcome and has can repeat (links from the Overview set them).
//
// Origins: the URL's `origin` values when it has some, else the remembered choice (default: what
// you typed and slash commands). The daemon itself shows every origin when none is sent.
//
// The filters are read here and returned at once; the prompts and their counts stream in.
export const load: PageLoad = ({ fetch, url }) => {
	const filters: Filters = {};
	for (const key of FILTER_KEYS) {
		const value = url.searchParams.get(key);
		if (value) filters[key] = value;
	}
	const lists = {
		model: url.searchParams.getAll('model'),
		outcome: url.searchParams.getAll('outcome') as TurnOutcome[],
		has: url.searchParams.getAll('has') as Has[]
	};
	const inUrl = url.searchParams.getAll('origin');
	const origins: Origin[] = inUrl.length ? parseOrigins(inUrl) : originFilter.origins;
	const limit = Math.min(MAX_LIMIT, Math.max(PAGE, Number(url.searchParams.get('limit')) || PAGE));

	const params = queryParams({ ...filters, ...lists, origin: origins });
	params.set('limit', String(limit));
	const { before: _, ...counted } = filters; // counts cover every page, not just older prompts
	// Each select's choices are counted with the other filters applied, not its own.
	const facet = (key: Facet) => {
		const { [key]: _, ...others } = { ...counted, ...lists, origin: origins } as PromptQuery;
		return api.stats({ ...others, group_by: key, order: 'prompts', limit: 100 }, fetch);
	};
	const listing = Promise.all([
		api.prompts(params, fetch),
		api.stats({ ...counted, ...lists, group_by: 'origin' }, fetch),
		Promise.all(PROMPT_FACETS.map(facet))
	]).then(([prompts, byOrigin, facets]) => ({
		prompts,
		originCounts: Object.fromEntries(byOrigin.groups.map((g) => [g.key, g.prompts])) as Partial<
			Record<Origin, number>
		>,
		facetOptions: Object.fromEntries(PROMPT_FACETS.map((f, i) => [f, facets[i]])) as Record<
			Facet,
			Stats
		>
	}));
	return { listing: defer(listing), filters, lists, origins, limit };
};
