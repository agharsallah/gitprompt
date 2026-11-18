/**
 * The tabs of a project page, as data: the page draws whatever this list holds, so a new tab is one
 * more entry here and a branch in ProjectTabs. The tab lives in the URL (`?tab=`), so it survives a
 * reload and the back button goes to the tab you were on.
 */
import { COMMIT_ANCHOR } from '#lib/projects/commits.ts';
import { projectHref } from '#lib/shared/links.ts';

export const PROJECT_TABS = [
	{ id: 'journey', label: 'Journey' },
	{ id: 'commits', label: 'Commits' }
] as const;

export type ProjectTab = (typeof PROJECT_TABS)[number]['id'];

const DEFAULT_TAB: ProjectTab = 'journey';

const isTab = (value: string | null): value is ProjectTab =>
	PROJECT_TABS.some((tab) => tab.id === value);

/**
 * The tab a URL opens: its `?tab=`, else Commits when it points at a commit's row (a link from the
 * palette may name the row alone), else the journey.
 */
export function tabOf(url: {
	searchParams: Pick<URLSearchParams, 'get'>;
	hash: string;
}): ProjectTab {
	const named = url.searchParams.get('tab');
	if (isTab(named)) return named;
	return url.hash.startsWith(`#${COMMIT_ANCHOR}`) ? 'commits' : DEFAULT_TAB;
}

/** The URL of a project's tab. The default tab is left out of the URL, as the page opens on it. */
export const tabHref = (project: string, tab: ProjectTab): string =>
	projectHref(project, { tab: tab === DEFAULT_TAB ? undefined : tab });

/** SvelteKit's `goto`, as far as a tab uses it. */
type Navigate = (href: string, options: { replace: boolean; reset: boolean }) => unknown;

/**
 * Open a tab: a new history entry (never a replaced one), so Back returns to the tab before. The
 * scroll position and focus stay where they are (`reset: false`), as with any tab.
 */
export const openTab = (project: string, tab: ProjectTab, go: Navigate) =>
	go(tabHref(project, tab), { replace: false, reset: false });
