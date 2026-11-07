import { recent } from '#lib/api/recent.ts';
import { parseRange } from '#lib/insights/range.ts';
import { preferences } from '#lib/settings/preferences.ts';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch, url }) => {
	const { default_range } = await preferences(fetch);
	const range = parseRange(url.searchParams.get('range'), default_range);
	return { range, defaultRange: default_range, window: await recent(range, fetch) };
};
