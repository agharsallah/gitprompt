/**
 * The sidebar's pages and what each is for, in one place: the sidebar shows a purpose line under
 * each link and the page's header states the same line under its title, so the two never drift.
 * The four everyday pages run from far to near (the zoom: Overview, Projects, Sessions, Prompts).
 * Pure, tested in nav.test.ts.
 */
import type { PageRoute } from '#lib/shared/links.ts';

/** The job each everyday page serves, in the words of the person opening it. */
export const PURPOSES = {
	'/': 'What needs you',
	'/projects': 'Where each project stands',
	'/sessions': 'Get back into a session',
	'/prompts': 'Find a prompt again'
} as const;

export type PurposeRoute = keyof typeof PURPOSES;

export type NavLink = { route: PageRoute; label: string; purpose?: string };
export type NavGroup = { label: string; links: readonly NavLink[] };

export const NAV: readonly NavGroup[] = [
	{
		label: 'Your work',
		links: [
			{ route: '/', label: 'Overview', purpose: PURPOSES['/'] },
			{ route: '/projects', label: 'Projects', purpose: PURPOSES['/projects'] },
			{ route: '/sessions', label: 'Sessions', purpose: PURPOSES['/sessions'] },
			{ route: '/prompts', label: 'Prompts', purpose: PURPOSES['/prompts'] }
		]
	},
	{
		label: 'Setup',
		links: [
			{ route: '/sources', label: 'Connectors' },
			{ route: '/fields', label: 'Fields' },
			{ route: '/settings', label: 'Settings' },
			{ route: '/design', label: 'Design system' }
		]
	}
];

/** Whether the page whose route id is `routeId` is this link's page (or one of its detail pages). */
export function isCurrent(link: Pick<NavLink, 'route'>, routeId: string | null): boolean {
	if (!routeId) return false;
	return routeId === link.route || (link.route !== '/' && routeId.startsWith(`${link.route}/`));
}

/** A link's tooltip: its purpose line when it has one (what the collapsed sidebar shows). */
export const tooltip = (link: NavLink): string =>
	link.purpose ? `${link.label}: ${link.purpose}` : link.label;
