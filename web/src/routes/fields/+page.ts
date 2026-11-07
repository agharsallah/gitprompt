import { api } from '#lib/api/client.ts';
import { defer } from '#lib/async/resource.svelte.ts';
import type { PageLoad } from './$types';

export const load: PageLoad = ({ fetch }) => ({
	content: defer(
		Promise.all([api.fields(fetch), api.sources(fetch)]).then(([catalog, sources]) => ({
			catalog,
			sources
		}))
	)
});
