import { redirect } from '@sveltejs/kit';
import { startView } from '#lib/settings/views.ts';
import type { LayoutLoad } from './$types';

// Static SPA served by the local daemon: every page is a prerendered shell, data loads in the browser.
export const prerender = true;
export const ssr = false;
export const trailingSlash = 'always';

let started = false;

// Opening GitPrompt (a new tab, the desktop app) at a page with no query shows the view you last
// left it in (lib/settings/views.ts). Only the first load: every later link shows what it says.
export const load: LayoutLoad = ({ route, url }) => {
	if (started) return;
	started = true;
	const search = startView(route.id, url.search);
	if (search) redirect(307, `${url.pathname}${search}`);
};
