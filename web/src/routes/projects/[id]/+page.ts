import { api } from '#lib/api/client.ts';
import { orNotFound } from '#lib/stories/load.ts';
import type { PageLoad } from './$types';

// A story per project id (lib/stories/story.ts projectId): served by the dashboard's fallback page.
export const prerender = false;

export const load: PageLoad = async ({ fetch, params }) => ({
	story: await orNotFound(api.projectStory(params.id, fetch), 'project')
});
