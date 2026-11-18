import { describe, expect, it } from 'vitest';
import {
	pageHref,
	projectHref,
	projectsHref,
	promptHref,
	promptListHref,
	sessionHref,
	sessionsHref,
	sourcesHref,
	targetHref
} from '#lib/shared/links.ts';

describe('pageHref', () => {
	it('is the page itself without a query or an anchor', () => {
		expect(pageHref('/')).toBe('/');
		expect(pageHref('/prompts')).toBe('/prompts');
	});

	it('adds a record query, encoded, leaving out undefined values', () => {
		expect(pageHref('/prompts', { query: { repo: '/a b/c', tag: undefined, id: 4 } })).toBe(
			'/prompts?repo=%2Fa%20b%2Fc&id=4'
		);
	});

	it('adds ready-made search params as they are, and nothing when they are empty', () => {
		expect(pageHref('/prompts', { query: new URLSearchParams([['q', 'a b']]) })).toBe(
			'/prompts?q=a+b'
		);
		expect(pageHref('/prompts', { query: new URLSearchParams() })).toBe('/prompts');
	});

	it('adds the anchor after the query', () => {
		expect(pageHref('/sources', { query: { connect: 'x' }, anchor: 'top' })).toBe(
			'/sources?connect=x#top'
		);
	});
});

describe('promptHref', () => {
	it('selects the prompt on the Prompts page', () => {
		expect(promptHref(42)).toBe('/prompts?id=42');
	});
});

describe('promptListHref', () => {
	it('lists the prompts one filter keeps, with the value encoded', () => {
		expect(promptListHref()).toBe('/prompts');
		expect(promptListHref({ session: 'a b' })).toBe('/prompts?session=a%20b');
		expect(promptListHref({ source: 'codex', surface: 'cli' })).toBe(
			'/prompts?source=codex&surface=cli'
		);
	});
});

describe('sessionHref', () => {
	it('opens the session story', () => {
		expect(sessionHref('abc')).toBe('/sessions/abc');
	});

	it('jumps to an anchor in it', () => {
		expect(sessionHref('abc', { anchor: 'prompt-7' })).toBe('/sessions/abc#prompt-7');
	});
});

describe('projectHref', () => {
	it('opens the project story', () => {
		expect(projectHref('p1')).toBe('/projects/p1');
	});

	it('keeps an anchor, a tab and a window', () => {
		expect(projectHref('p1', { anchor: 'prompt-7' })).toBe('/projects/p1#prompt-7');
		expect(
			projectHref('p1', { tab: 'commits', from: '2026-01-01', to: '2026-02-01', anchor: 'a' })
		).toBe('/projects/p1?tab=commits&from=2026-01-01&to=2026-02-01#a');
	});

	it('carries the range the list was on, so the way back opens the same days', () => {
		expect(projectHref('p1', { range: '24h' })).toBe('/projects/p1?range=24h');
		expect(projectHref('p1', { tab: 'commits', range: '30d' })).toBe(
			'/projects/p1?tab=commits&range=30d'
		);
	});
});

describe('projectsHref', () => {
	it('opens the Projects list, on a range when one is given', () => {
		expect(projectsHref()).toBe('/projects');
		expect(projectsHref({ range: '24h' })).toBe('/projects?range=24h');
	});
});

describe('sessionsHref', () => {
	it('lists sessions, of a project, or at one session', () => {
		expect(sessionsHref()).toBe('/sessions');
		expect(sessionsHref({ project: '/a/b' })).toBe('/sessions?project=%2Fa%2Fb');
		expect(sessionsHref({ session: 's1' })).toBe('/sessions#session-s1');
	});
});

describe('sourcesHref', () => {
	it('opens Connectors, a tool setup, or a tool card', () => {
		expect(sourcesHref()).toBe('/sources');
		expect(sourcesHref({ connect: 'codex' })).toBe('/sources?connect=codex');
		expect(sourcesHref({ tool: 'codex' })).toBe('/sources#tool-codex');
	});
});

describe('targetHref', () => {
	it('leads to the page an attention item names', () => {
		expect(targetHref({ page: 'overview' })).toBe('/');
		expect(targetHref({ page: 'prompts', q: 'fix lint' })).toBe('/prompts?q=fix%20lint');
		expect(targetHref({ page: 'prompt', id: 7 })).toBe('/prompts?id=7');
		expect(targetHref({ page: 'session', id: 's1' })).toBe('/sessions/s1');
		expect(targetHref({ page: 'leaks' })).toBe('/#leaks');
	});
});
