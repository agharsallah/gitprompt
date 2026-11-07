import { api } from '#lib/api/client.ts';
import { recent } from '#lib/api/recent.ts';
import { parseRange } from '#lib/insights/range.ts';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch, url }) => {
	const range = parseRange(url.searchParams.get('range'));
	const [window, sources] = await Promise.all([recent(range, fetch), api.sources(fetch)]);
	return { range, window, sources };
};
