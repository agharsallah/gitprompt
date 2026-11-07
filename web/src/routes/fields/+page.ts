import { api } from '#lib/api/client.ts';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch }) => {
	const [catalog, sources] = await Promise.all([api.fields(fetch), api.sources(fetch)]);
	return { catalog, sources };
};
