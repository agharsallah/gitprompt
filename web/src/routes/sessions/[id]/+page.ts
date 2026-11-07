import { api } from '#lib/api/client.ts';
import { orNotFound } from '#lib/stories/load.ts';
import type { PageLoad } from './$types';

// A story per session id: not prerendered, the daemon serves the dashboard's fallback page.
export const prerender = false;

export const load: PageLoad = async ({ fetch, params }) => ({
	story: await orNotFound(api.sessionStory(params.id, fetch), 'session')
});
