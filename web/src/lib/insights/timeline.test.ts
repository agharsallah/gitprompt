import { describe, expect, it } from 'vitest';
import { eventTime, toolDetail, turnTimeline } from '#lib/insights/timeline.ts';
import { after, event } from '#lib/testing/builders.ts';

const start = '2026-11-05T10:00:00.000Z';

describe('turnTimeline', () => {
	it('places a tool call from its duration back from when it finished', () => {
		const t = turnTimeline(
			[
				event({
					event: 'PostToolUse',
					received_at: after(start, 10),
					raw: { tool_name: 'Bash', duration_ms: '4000', tool_input: { command: 'pnpm test' } }
				})
			],
			start
		);
		expect(t.spans).toEqual([
			expect.objectContaining({
				row: 'shell',
				label: 'Bash',
				detail: 'pnpm test',
				start: 6,
				end: 10,
				failed: false
			})
		]);
	});

	it('takes model calls from telemetry, timed by event.timestamp', () => {
		const t = turnTimeline(
			[
				event({
					event: 'api_request',
					received_at: after(start, 60),
					raw: {
						'event.timestamp': after(start, 5),
						duration_ms: '2000',
						model: 'claude-opus-5-5',
						query_source: 'repl_main_thread'
					}
				})
			],
			start
		);
		expect(t.spans[0]).toMatchObject({
			row: 'model',
			label: 'Opus 5.5',
			detail: 'repl main thread',
			start: 3,
			end: 5
		});
	});

	it('marks failed tool calls', () => {
		const t = turnTimeline(
			[
				event({
					event: 'PostToolUseFailure',
					received_at: after(start, 2),
					raw: { tool_name: 'Edit' }
				})
			],
			start
		);
		expect(t.spans[0]).toMatchObject({ row: 'write', failed: true });
	});

	it('lists only the rows that have calls, model first, then kinds in order', () => {
		const t = turnTimeline(
			[
				event({ received_at: after(start, 3), raw: { tool_name: 'Bash' } }),
				event({ received_at: after(start, 2), raw: { tool_name: 'Read' } }),
				event({ event: 'api_request', raw: { 'event.timestamp': after(start, 1) } })
			],
			start
		);
		expect(t.rows).toEqual(['model', 'read', 'shell']);
		expect(t.spans.map((s) => s.label)).toEqual(['Model call', 'Read', 'Bash']);
	});

	it('ends at the turn and only counts calls made after it ended', () => {
		const t = turnTimeline(
			[
				event({ received_at: after(start, 8), raw: { tool_name: 'Bash' } }),
				event({
					event: 'api_request',
					raw: { 'event.timestamp': after(start, 300), duration_ms: '500' }
				})
			],
			start,
			10
		);
		expect(t.end).toBe(10);
		expect(t.spans).toHaveLength(1);
		expect(t.after).toBe(1);
	});

	it('ignores events that are neither tool results nor model calls', () => {
		const t = turnTimeline([event({ event: 'Notification' }), event({ event: 'Stop' })], start, 5);
		expect(t.spans).toEqual([]);
		expect(t.end).toBe(5);
	});

	it('never starts a span before the prompt', () => {
		const t = turnTimeline(
			[event({ received_at: after(start, 1), raw: { tool_name: 'Read', duration_ms: '5000' } })],
			start
		);
		expect(t.spans[0].start).toBe(0);
	});
});

describe('toolDetail', () => {
	it.each([
		[{ command: 'git status\ngit diff' }, 'git status'],
		[{ file_path: '/a/b.ts', content: '...' }, '/a/b.ts'],
		[{ url: 'https://example.com' }, 'https://example.com'],
		[{ pattern: 'TODO' }, 'TODO'],
		['{"query": "svelte runes"}', 'svelte runes'],
		['not json', 'not json'],
		[null, ''],
		[{ other: 1 }, '']
	])('%j -> %s', (input, detail) => expect(toolDetail(input)).toBe(detail));
});

describe('eventTime', () => {
	it('prefers when telemetry says it happened over when it arrived', () => {
		const e = event({ received_at: after(start, 60), raw: { 'event.timestamp': after(start, 1) } });
		expect(eventTime(e)).toBe(Date.parse(after(start, 1)));
		expect(eventTime(event({ received_at: start }))).toBe(Date.parse(start));
	});
});
