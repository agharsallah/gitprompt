import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import ProjectRow from '#lib/projects/ProjectRow.svelte';
import type { ProjectSummary } from '#lib/projects/summary.ts';
import { projectHref } from '#lib/shared/links.ts';
import { projectId } from '#lib/stories/story.ts';

const summary = (overrides: Partial<ProjectSummary> = {}): ProjectSummary => ({
	key: '/r/app',
	name: 'me/app',
	repo: 'me/app',
	git: true,
	prompts: 95,
	typed: 73,
	fromAgents: 22,
	sessions: 21,
	cost: 0,
	lastAt: '2026-11-18T09:47:40Z',
	branches: [],
	filesWritten: 0,
	failed: 0,
	kinds: [],
	daily: [],
	...overrides
});

const html = (s: ProjectSummary, range = '24h') =>
	render(ProjectRow, { props: { summary: s, share: 1, range } }).body;

describe('a project row (ProjectRow)', () => {
	it('opens the project on the range the list shows', () => {
		const href = projectHref(projectId('/r/app'), { range: '24h' });
		expect(href).toContain('range=24h');
		expect(html(summary())).toContain(`href="${href}"`);
	});

	it('says how many prompts you typed and how many agents sent, as the project page counts them', () => {
		const out = html(summary());
		expect(out).toContain('73 typed');
		expect(out).toContain('22 from agents');
	});

	it('says nothing about agents when every prompt was typed', () => {
		const out = html(summary({ prompts: 73, typed: 73, fromAgents: 0 }));
		expect(out).not.toContain('from agents');
	});
});
