import { recent } from '#lib/api/recent.ts';
import { parseRange } from '#lib/insights/range.ts';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch, url }) => {
	const range = parseRange(url.searchParams.get('range'));
	return { range, window: await recent(range, fetch) };
};
