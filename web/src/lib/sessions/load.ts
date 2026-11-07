/**
 * The Sessions page's data: which sessions to show comes from /api/stats (searched, filtered and
 * sorted over every prompt, no cap); their turns from /api/prompts by session id.
 */
import { api } from '#lib/api/client.ts';
import { queryParams, type StatsQuery } from '#lib/api/query.ts';
import type { Prompt, StatsOrder } from '#lib/api/types.ts';

type Fetch = typeof fetch;

export const SORTS = {
	recent: { label: 'Recent', order: 'last_at' },
	longest: { label: 'Most turns', order: 'prompts' },
	costliest: { label: 'Costliest', order: 'cost_usd' }
} as const satisfies Record<string, { label: string; order: StatsOrder }>;
export type Sort = keyof typeof SORTS;
export const isSort = (value: string | null): value is Sort => !!value && value in SORTS;

/** Sessions shown at first, and added by "Show more". */
export const SESSION_PAGE = 20;
const PAGE = 200; // the prompt list's page size limit
const MAX_PAGES = 20;

/** Every prompt of these sessions that the query keeps, oldest pages last. */
export async function promptsOf(
	ids: string[],
	query: StatsQuery,
	fetcher: Fetch
): Promise<Prompt[]> {
	if (!ids.length) return [];
	const out: Prompt[] = [];
	let before: number | undefined;
	for (let i = 0; i < MAX_PAGES; i++) {
		const params = queryParams({ ...query, session: ids, limit: PAGE, before: before?.toString() });
		const page = await api.prompts(params, fetcher);
		out.push(...page);
		if (page.length < PAGE) break;
		before = page.at(-1)!.id;
	}
	return out;
}
