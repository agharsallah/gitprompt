import { api } from '#lib/api/client.ts';
import { FILTER_KEYS, type Filters } from '#lib/api/types.ts';
import { MAX_LIMIT, PAGE } from '#lib/prompts/paging.ts';
import type { PageLoad } from './$types';

// Only the server-side filters (and the page size) are read here, so SvelteKit reruns this load
// when one of them changes, and not when a prompt is selected (?id=) or a quick filter (?show=)
// is toggled: those are handled in the page component.
export const load: PageLoad = async ({ fetch, url }) => {
	const filters: Filters = {};
	const params = new URLSearchParams();
	for (const key of FILTER_KEYS) {
		const value = url.searchParams.get(key);
		if (value) params.set(key, (filters[key] = value));
	}
	const limit = Math.min(MAX_LIMIT, Math.max(PAGE, Number(url.searchParams.get('limit')) || PAGE));
	params.set('limit', String(limit));
	return { prompts: await api.prompts(params, fetch), filters, limit };
};
