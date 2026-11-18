import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { Chapter } from '#lib/api/types.ts';
import ChapterView from '#lib/stories/Chapter.svelte';
import { promptHref } from '#lib/shared/links.ts';

const chapter: Chapter = {
	id: 42,
	turn: 3,
	received_at: '2026-11-05T10:00:00Z',
	origin: 'typed',
	text: 'Make the copy button clearer',
	clipped: false,
	slash_command: null,
	branch: null,
	outcome: 'done',
	response_s: 12,
	cost_usd: 0,
	tokens: 0,
	tool_calls: 0,
	tool_failures: 0,
	tools: {},
	files_written: 0,
	files: [],
	subagents: 0,
	git_commits: 0,
	commits: [],
	diffstat: null,
	retry_of: null,
	facts: null
};

describe('a chapter of a session story', () => {
	it('says plainly how to open its prompt, besides the turn number', () => {
		const out = render(ChapterView, { props: { chapter } }).body;
		const link = out.slice(out.lastIndexOf('<a', out.indexOf('Open prompt')));
		expect(link).toContain(`href="${promptHref(42)}"`);
		expect(out).toContain('title="Prompt › Turn 3"');
	});
});
