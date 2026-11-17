import type { Component, ComponentProps } from 'svelte';
import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { Resume } from '#lib/api/types.ts';
import { sessions, type SessionGroup } from '#lib/insights/aggregate.ts';
import ResumeCommand from '#lib/sessions/ResumeCommand.svelte';
import RunningNow from '#lib/sessions/RunningNow.svelte';
import SessionCard from '#lib/sessions/SessionCard.svelte';
import SessionStory from '#lib/stories/SessionStory.svelte';
import { prompt, sessionStory } from '#lib/testing/builders.ts';

const resume: Resume = {
	command: 'claude --resume session-a',
	cwd: '/work/app',
	paste: 'cd /work/app && claude --resume session-a'
};
const COPY = /aria-label="Copy[^"]*"/;

const html = <P extends Record<string, unknown>>(component: Component<P>, props: P) =>
	render(component, { props }).body;

describe('ResumeCommand', () => {
	it('shows the line to paste with a copy button', () => {
		const out = html(ResumeCommand, { resume });
		expect(out).toContain('cd /work/app &amp;&amp; claude --resume session-a');
		expect(out).toContain('aria-label="Copy Resume in a terminal"');
	});

	it('is only the button when compact, with the line as its tooltip', () => {
		const out = html(ResumeCommand, { resume, compact: true });
		expect(out).toContain('aria-label="Copy resume command"');
		expect(out).toContain('title="cd /work/app &amp;&amp; claude --resume session-a"');
		expect(out).not.toContain('<pre');
	});

	it('is hidden without a resume command', () => {
		for (const props of [{}, { resume: null }, { resume: null, compact: true }]) {
			expect(html(ResumeCommand, props)).not.toMatch(COPY);
		}
	});
});

describe('a session row (SessionCard)', () => {
	const card = (group: SessionGroup, at?: string) => {
		const [session] = sessions(
			[prompt({ received_at: at ?? '2026-11-05T10:00:00Z', fields: { session_id: 's1' } })],
			new Map([['s1', group]])
		);
		return html(SessionCard, { session });
	};

	it('has the compact copy button when its tool can resume it', () => {
		expect(card({ resume })).toContain('aria-label="Copy resume command"');
	});

	it('has none for a tool without a template', () => {
		expect(card({})).not.toContain('Copy resume command');
	});
});

describe('a session story (SessionStory)', () => {
	it('shows the block under its header', () => {
		const out = html(SessionStory, { story: sessionStory({ resume }) });
		expect(out).toContain('cd /work/app &amp;&amp; claude --resume session-a');
		expect(out).toContain('aria-label="Copy Resume in a terminal"');
		expect(out.indexOf('Pick this session up again')).toBeGreaterThan(out.indexOf('<h1'));
	});

	it('says which tool has no resume command', () => {
		const cursor = { id: 'cursor', name: 'Cursor', label: 'Cursor', assistant: 'Cursor' };
		const out = html(SessionStory, { story: sessionStory({ connector: cursor }) });
		expect(out).toContain('Cursor has no resume command');
		expect(out).not.toContain('Copy Resume in a terminal');
	});
});

describe('RunningNow', () => {
	const NOW = Date.parse('2026-11-05T10:00:00Z');
	const lastEventAt = new Date(NOW - 12_000).toISOString();

	it('says a session is running, and since when it last did anything', () => {
		const out = html(RunningNow, { lastEventAt, now: NOW, stream: 'open' });
		expect(out).toContain('Running · last event 12 s ago');
	});

	it('says nothing about a session that is left, ended or unwatched', () => {
		const long = new Date(NOW - 3_600_000).toISOString();
		const left: ComponentProps<typeof RunningNow>[] = [
			{ lastEventAt: long, now: NOW, stream: 'open' },
			{ lastEventAt, ended: true, now: NOW, stream: 'open' },
			{ lastEventAt, now: NOW, stream: 'closed' }
		];
		for (const props of left) {
			expect(html(RunningNow, props)).not.toContain('Running');
		}
	});
});
