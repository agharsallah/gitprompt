import { api } from '#lib/api/client.ts';
import { defer } from '#lib/async/resource.svelte.ts';
import type { StatsBucket } from '#lib/api/types.ts';
import type { PageLoad } from './$types';

async function connectors(fetcher: typeof fetch) {
	const [sources, problems, recent] = await Promise.all([
		api.sources(fetcher),
		api.specProblems(fetcher),
		api.recentCaptures(fetcher)
	]);
	// Prompts by origin for each connector that captured some: shows its origin rules at work.
	const counted = sources.filter((s) => s.total > 0);
	const stats = await Promise.all(
		counted.map((s) => api.stats({ source: s.id, group_by: 'origin' }, fetcher))
	);
	const origins: Record<string, StatsBucket[]> = Object.fromEntries(
		counted.map((s, i) => [s.id, stats[i].groups])
	);
	return { sources, origins, problems, recent };
}

export const load: PageLoad = ({ fetch }) => ({ connectors: defer(connectors(fetch)) });
