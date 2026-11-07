import { api } from '#lib/api/client.ts';
import type { StatsBucket } from '#lib/api/types.ts';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch }) => {
	const [sources, problems] = await Promise.all([api.sources(fetch), api.specProblems(fetch)]);
	// Prompts by origin for each connector that captured some: shows its origin rules at work.
	const counted = sources.filter((s) => s.total > 0);
	const stats = await Promise.all(
		counted.map((s) => api.stats({ source: s.id, group_by: 'origin' }, fetch))
	);
	const origins: Record<string, StatsBucket[]> = Object.fromEntries(
		counted.map((s, i) => [s.id, stats[i].groups])
	);
	return { sources, origins, problems };
};
