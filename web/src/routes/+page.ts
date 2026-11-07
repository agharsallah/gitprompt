import { api } from '#lib/api/client.ts';
import { recent } from '#lib/api/recent.ts';
import { parseRange } from '#lib/insights/range.ts';
import { preferences } from '#lib/settings/preferences.ts';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch, url }) => {
	const { default_range, week_start } = await preferences(fetch);
	const range = parseRange(url.searchParams.get('range'), default_range);
	const [window, sources] = await Promise.all([recent(range, fetch), api.sources(fetch)]);
	return { range, defaultRange: default_range, weekStart: week_start, window, sources };
};
