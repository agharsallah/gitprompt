/**
 * The old Search page lived at `/search?q=&repo=&tag=&range=&origin=&limit=`; Search is now the text
 * box of Prompts, which reads the same parameters under the same names. This turns an old URL's
 * query into the Prompts page's, so a bookmark shows the same prompts. Pure, tested in
 * fromSearch.test.ts.
 */
import { parseRange, type RangeId } from '#lib/insights/range.ts';
import { MAX_LIMIT, PAGE } from '#lib/prompts/paging.ts';

/** Marks a visit that came from the old Search page, so Prompts can say where Search went. */
export const MOVED_PARAM = 'from';
export const MOVED_VALUE = 'search';

/**
 * The filters an old Search query carries over, under the names Prompts reads: words (`q`),
 * project (`repo`), `tag`, every `origin`, and how many rows (`limit`).
 */
export function carriedFilters(search: string | URLSearchParams): URLSearchParams {
	const old = new URLSearchParams(search);
	const carried = new URLSearchParams();
	for (const key of ['q', 'repo', 'tag'] as const) {
		const value = old.get(key)?.trim();
		if (value) carried.set(key, value);
	}
	for (const origin of old.getAll('origin')) carried.append('origin', origin);
	const limit = Number(old.get('limit'));
	if (limit > PAGE) carried.set('limit', String(Math.min(MAX_LIMIT, Math.floor(limit))));
	return carried;
}

/**
 * The Prompts query for an old Search query. `defaultRange` is the `default_range` setting: Search
 * opened at it when its URL named no (or an unknown) range, so it is named here, as Prompts alone
 * would open at all time. Origins are left out when the URL has none, which both pages read as
 * "the origins remembered in this browser".
 */
export function promptsQueryFromSearch(
	search: string | URLSearchParams,
	defaultRange: RangeId
): URLSearchParams {
	const range = parseRange(new URLSearchParams(search).get('range'), defaultRange).id;
	const query = new URLSearchParams({ range });
	for (const [key, value] of carriedFilters(search)) query.append(key, value);
	query.set(MOVED_PARAM, MOVED_VALUE);
	return query;
}
