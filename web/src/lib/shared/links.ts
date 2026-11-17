/**
 * The one place that builds the dashboard's URLs: a page, a prompt, a session, a project, a commit.
 * Every link in the app goes through here (links.guard.test.ts fails when a route string appears
 * anywhere else), and later deep links map through the same functions. Pure; `resolve` only adds
 * the app's base path.
 */
import { resolve } from '$app/paths';

/** The pages that take no path parameter. */
export type PageRoute =
	'/' | '/prompts' | '/sessions' | '/projects' | '/sources' | '/fields' | '/settings' | '/design';

/**
 * Each page's SvelteKit route id, by the name the page goes by: what reads the route a page is on
 * (usage counts) maps it through here, as links are built through the functions below.
 */
export const ROUTE_IDS = {
	overview: '/',
	prompts: '/prompts',
	sessions: '/sessions',
	session: '/sessions/[id]',
	projects: '/projects',
	project: '/projects/[id]',
	sources: '/sources',
	fields: '/fields',
	settings: '/settings'
} as const;

type QueryValue = string | number | undefined;
/** A query: ready-made search params, or a record (an undefined value is left out). */
export type Query = URLSearchParams | Record<string, QueryValue>;

/** `?a=1&b=2`, or nothing when there is no parameter. */
function queryString(query?: Query): string {
	if (query === undefined) return '';
	const text =
		query instanceof URLSearchParams
			? query.toString()
			: Object.entries(query)
					.filter((entry): entry is [string, string | number] => entry[1] !== undefined)
					.map(([key, value]) => `${key}=${encodeURIComponent(value)}`)
					.join('&');
	return text ? `?${text}` : '';
}

const hashOf = (anchor?: string) => (anchor ? `#${anchor}` : '');

/** A URL (a page's, from `pageHref`) with a query added. */
export const withQuery = (href: string, query: Query) => `${href}${queryString(query)}`;

/** A page's URL, with an optional query and an optional `#anchor` (given without the `#`). */
export function pageHref(
	route: PageRoute,
	{ query, anchor }: { query?: Query; anchor?: string } = {}
): string {
	return `${resolve(route)}${queryString(query)}${hashOf(anchor)}`;
}

/** The Prompts page with this prompt selected. */
export const promptHref = (id: number) => pageHref('/prompts', { query: { id } });

/** The Prompts page listing what one filter keeps (`repo`, `session`, `tag`, `source`...). */
export const promptListHref = (query: Query = {}) => pageHref('/prompts', { query });

/** A session's story; `anchor` is an element id in it (a prompt's `prompt-<id>`). */
export function sessionHref(id: string, { anchor }: { anchor?: string } = {}): string {
	return `${resolve('/sessions/[id]', { id })}${hashOf(anchor)}`;
}

/**
 * A project's story. `tab` names one of its tabs, `from`/`to` bound the time window shown
 * (ISO dates) and `anchor` is an element id in it.
 */
export function projectHref(
	id: string,
	{ tab, anchor, from, to }: { tab?: string; anchor?: string; from?: string; to?: string } = {}
): string {
	return `${resolve('/projects/[id]', { id })}${queryString({ tab, from, to })}${hashOf(anchor)}`;
}

/**
 * A commit on its remote's site (`remote` as the daemon reports it, e.g. https://host/owner/name);
 * nothing without a remote, since a hash alone leads nowhere.
 */
export function commitHref(hash: string, remote?: string): string | undefined {
	return remote ? `${remote.replace(/\/+$/, '')}/commit/${encodeURIComponent(hash)}` : undefined;
}

/** The Sessions list: narrowed to a project, or scrolled to a session's card. */
export function sessionsHref({ project, session }: { project?: string; session?: string } = {}) {
	return pageHref('/sessions', {
		query: { project },
		anchor: session === undefined ? undefined : `session-${session}`
	});
}

/** The Connectors page: opening a tool's setup (`connect`) or scrolled to its card (`tool`). */
export function sourcesHref({ connect, tool }: { connect?: string; tool?: string } = {}) {
	return pageHref('/sources', {
		query: { connect },
		anchor: tool === undefined ? undefined : `tool-${tool}`
	});
}
