import { describe, expect, it } from 'vitest';
import { isCurrent, NAV, PURPOSES, tooltip } from '#lib/layout/nav.ts';

const everyday = NAV[0].links;

describe('the sidebar', () => {
	it('runs the everyday pages from far to near, as the zoom does', () => {
		expect(everyday.map((l) => l.label)).toEqual(['Overview', 'Projects', 'Sessions', 'Prompts']);
		expect(everyday.map((l) => l.route)).toEqual(['/', '/projects', '/sessions', '/prompts']);
	});

	it('says what each everyday page is for, in the words of the person opening it', () => {
		expect(everyday.map((l) => l.purpose)).toEqual([
			'What needs you',
			'Where each project stands',
			'Get back into a session',
			'Find a prompt again'
		]);
		expect(everyday.every((l) => l.purpose === PURPOSES[l.route as keyof typeof PURPOSES])).toBe(
			true
		);
	});

	it('has no link to the old Search page: Find opens the palette instead', () => {
		const routes = NAV.flatMap((g) => g.links.map((l) => l.route as string));
		expect(routes).not.toContain('/search');
	});

	it('keeps the setup pages, without a purpose line', () => {
		expect(NAV[1].links.map((l) => l.route)).toEqual([
			'/sources',
			'/fields',
			'/settings',
			'/design'
		]);
		expect(NAV[1].links.every((l) => l.purpose === undefined)).toBe(true);
	});
});

describe('the page you are on', () => {
	const prompts = { route: '/prompts' } as const;

	it('is marked on its own route and on the pages under it', () => {
		expect(isCurrent(prompts, '/prompts')).toBe(true);
		expect(isCurrent({ route: '/sessions' }, '/sessions/[id]')).toBe(true);
		expect(isCurrent(prompts, '/sessions')).toBe(false);
		expect(isCurrent(prompts, null)).toBe(false);
	});

	it('is not the Overview for every page below the root', () => {
		expect(isCurrent({ route: '/' }, '/')).toBe(true);
		expect(isCurrent({ route: '/' }, '/projects')).toBe(false);
	});
});

describe('tooltip', () => {
	it('is the purpose line with the name, or just the name', () => {
		expect(tooltip({ route: '/', label: 'Overview', purpose: 'What needs you' })).toBe(
			'Overview: What needs you'
		);
		expect(tooltip({ route: '/settings', label: 'Settings' })).toBe('Settings');
	});
});
