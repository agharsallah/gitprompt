import { api } from '#lib/api/client.ts';
import { defer } from '#lib/async/resource.svelte.ts';
import { readFilters, toQuery, without } from '#lib/insights/filters.ts';
import { sessions } from '#lib/insights/aggregate.ts';
import { originFilter } from '#lib/prompts/originFilter.svelte.ts';
import { isSort, promptsOf, SESSION_PAGE, SORTS } from '#lib/sessions/load.ts';
import { preferences } from '#lib/settings/preferences.ts';
import type { PageLoad } from './$types';

// URL: ?range=, ?q= (sessions with a prompt matching every word), ?project= (a repo filter value),
// ?sort=recent|longest|costliest, ?limit= (sessions shown). Origins: the remembered choice.
// The controls come from the URL at once; the sessions stream in.
export const load: PageLoad = async ({ fetch, url }) => {
	const { default_range } = await preferences(fetch);
	const params = url.searchParams;
	const filters = readFilters(params, default_range, originFilter.origins);
	const sort = isSort(params.get('sort')) ? params.get('sort')! : 'recent';
	const q = params.get('q') ?? '';
	const project = params.get('project') || undefined;
	const limit = Math.min(500, Math.max(SESSION_PAGE, Number(params.get('limit')) || SESSION_PAGE));
	const base = { ...toQuery(filters, Date.now()), repo: project };

	const content = (async () => {
		const [list, projects] = await Promise.all([
			api.stats(
				{ ...base, q, group_by: 'session', order: SORTS[sort as keyof typeof SORTS].order, limit },
				fetch
			),
			api.stats({ ...without(base, 'repo'), q, group_by: 'repo', limit: 100 }, fetch)
		]);
		const ids = list.groups.map((g) => g.key).filter((k): k is string => !!k);
		const byId = new Map(sessions(await promptsOf(ids, base, fetch)).map((s) => [s.id, s]));
		return {
			total: list.totals.sessions,
			sessions: ids.map((id) => byId.get(id)).filter((s) => !!s),
			projects: projects.groups.filter((g) => g.key)
		};
	})();
	return {
		filters,
		sort,
		q,
		project,
		limit,
		content: defer(content)
	};
};
