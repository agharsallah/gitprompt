import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { ProjectStory } from '#lib/api/types.ts';
import ProjectHeader from '#lib/stories/ProjectHeader.svelte';

const story = (system: number): ProjectStory => ({
	id: 'p1',
	repo: '/r/app',
	name: 'me/app',
	git: true,
	system,
	phases: [],
	totals: {
		prompts: 73,
		sessions: 21,
		cost_usd: 0,
		tool_calls: 0,
		files_written: 0,
		outcomes: {},
		diffstat: null,
		retries: 0,
		median_chars: 0,
		with_mention: 0,
		with_code: 0,
		first_at: '2026-11-17T19:15:27Z',
		last_at: '2026-11-18T09:47:40Z'
	}
});

describe('a project header (ProjectHeader)', () => {
	it('shows the prompts agents sent beside the ones you typed, adding up to the list’s count', () => {
		const out = render(ProjectHeader, { props: { story: story(22) } }).body;
		expect(out).toContain('22 from agents');
		expect(out).toContain('95 in all');
	});

	it('says nothing about agents when none sent a prompt', () => {
		const out = render(ProjectHeader, { props: { story: story(0) } }).body;
		expect(out).not.toContain('from agents');
	});
});
