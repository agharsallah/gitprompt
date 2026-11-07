import { api } from '#lib/api/client.ts';
import { defer } from '#lib/async/resource.svelte.ts';
import { orNotFound } from '#lib/stories/load.ts';
import type { PageLoad } from './$types';

// A story per session id: not prerendered, the daemon serves the dashboard's fallback page.
export const prerender = false;

// The story streams in; an unknown id shows "not found" where the story would be.
export const load: PageLoad = ({ fetch, params }) => ({
	id: params.id,
	story: defer(orNotFound(api.sessionStory(params.id, fetch), 'session'))
});
