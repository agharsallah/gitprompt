// "Go to <page>": the layout's own commands for the pages in the sidebar.
import { goto } from '$app/navigation';
import { registerCommand } from '#lib/palette/commands.ts';
import { pageHref, type PageRoute } from '#lib/shared/links.ts';

const PAGES: { route: PageRoute; title: string; keywords: string[] }[] = [
	{ route: '/', title: 'Go to Overview', keywords: ['home', 'dashboard'] },
	{ route: '/prompts', title: 'Go to Prompts', keywords: ['search', 'find'] },
	{ route: '/sessions', title: 'Go to Sessions', keywords: ['conversations', 'resume'] },
	{ route: '/projects', title: 'Go to Projects', keywords: ['repositories'] },
	{ route: '/sources', title: 'Go to Connectors', keywords: ['tools', 'sources'] },
	{ route: '/settings', title: 'Go to Settings', keywords: ['preferences', 'privacy'] }
];

for (const { route, title, keywords } of PAGES) {
	registerCommand({ id: `go.${route}`, title, keywords, run: () => goto(pageHref(route)) });
}
