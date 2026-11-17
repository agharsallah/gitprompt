import { redirect } from '@sveltejs/kit';
import { promptsQueryFromSearch } from '#lib/prompts/fromSearch.ts';
import { preferences } from '#lib/settings/preferences.ts';
import { pageHref } from '#lib/shared/links.ts';
import type { PageLoad } from './$types';

// Search moved into the Prompts page. This route stays for old bookmarks: it sends the words,
// project, range, origins and tag to Prompts (lib/prompts/fromSearch.ts), which has no page of
// its own here, so the page file is empty.
export const load: PageLoad = async ({ fetch, url }) => {
	const { default_range } = await preferences(fetch);
	redirect(
		307,
		pageHref('/prompts', { query: promptsQueryFromSearch(url.searchParams, default_range) })
	);
};
