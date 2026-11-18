import { render } from 'svelte/server';
import { describe, expect, it, vi } from 'vitest';
import PickUp from '#lib/overview/PickUp.svelte';
import { pickUp } from '#lib/testing/builders.ts';

// The copy itself is CopyButton's (clipboard.ts); here, what Pick up hands it.
vi.mock('#lib/ui/CopyButton.svelte', async () => import('#lib/testing/CopyProbe.svelte'));

const NOW = Date.parse('2026-11-05T12:00:00Z');
const html = (rows?: ReturnType<typeof pickUp>[]) =>
	render(PickUp, { props: { rows, now: NOW } }).body;
const resume = { command: 'claude --resume s1', paste: 'cd /work && claude --resume s1' };

describe('PickUp', () => {
	it('shows at most three sessions, then the way to all of them', () => {
		const rows = ['s1', 's2', 's3', 's4'].map((session) =>
			pickUp({ session, name: `Name ${session}` })
		);
		const out = html(rows);
		expect(out.match(/<li class="row/g)).toHaveLength(3);
		expect(out).not.toContain('Name s4');
		expect(out).toMatch(/href="\/sessions"[^>]*>(<!---->)?All sessions/);
	});

	it('links a row to the session story, named, with its project and how it stopped', () => {
		const out = html([
			pickUp({ session: 's1', name: undefined, label: 'Fix the parser', ended: 'interrupted' })
		]);
		expect(out).toContain('href="/sessions/s1"');
		expect(out).toContain('Fix the parser');
		expect(out).toContain('>app<');
		expect(out).toContain('left 2 h ago, interrupted');
	});

	it('marks a running session with its own glyph and line', () => {
		const out = html([
			pickUp({ presence: 'running', ended: 'running', last_event_at: '2026-11-05T11:58:00Z' })
		]);
		expect(out).toMatch(/class="glyph running/);
		expect(out).toContain('running · last event 2 min ago');
	});

	it('copies the resume command counted as pickup.copy', () => {
		const out = html([pickUp({ resume })]);
		expect(out).toContain('data-copy="cd /work &amp;&amp; claude --resume s1"');
		expect(out).toContain('data-count="pickup.copy"');
	});

	it('has no copy for a session its tool cannot resume', () => {
		expect(html([pickUp()])).not.toContain('data-copy');
	});

	it('says there is nothing to pick up, calmly', () => {
		expect(html([])).toContain('Nothing to pick up');
	});

	it('draws a skeleton while the rows are on their way', () => {
		const out = html(undefined);
		expect(out).toContain('Loading the sessions to pick up');
		expect(out).not.toContain('All sessions');
	});
});
