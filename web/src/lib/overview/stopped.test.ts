import { describe, expect, it } from 'vitest';
import type { TurnOutcome } from '#lib/api/types.ts';
import { sinceText, stoppedText } from '#lib/overview/stopped.ts';
import { pickUp } from '#lib/testing/builders.ts';

const NOW = Date.parse('2026-11-05T12:00:00Z');
const ago = (ms: number) => new Date(NOW - ms).toISOString();
const MIN = 60_000;
const HOUR = 60 * MIN;

describe('stoppedText', () => {
	it('says a running session is running, with the age of its last event', () => {
		const row = pickUp({ presence: 'running', ended: 'running', last_event_at: ago(2 * MIN) });
		expect(stoppedText(row, NOW)).toBe('running · last event 2 min ago');
	});

	it.each<[TurnOutcome, string]>([
		['done', 'left 3 h ago, finished'],
		['failed', 'left 3 h ago, failed'],
		['interrupted', 'left 3 h ago, interrupted'],
		['open', 'left 3 h ago, mid-turn'],
		['running', 'left 3 h ago, mid-turn'],
		['superseded', 'left 3 h ago, mid-turn']
	])('says a session left after a %s turn: "%s"', (ended, line) => {
		expect(stoppedText(pickUp({ ended, last_event_at: ago(3 * HOUR) }), NOW)).toBe(line);
	});

	it('leaves the age out when the time cannot be read', () => {
		expect(stoppedText(pickUp({ last_event_at: 'never' }), NOW)).toBe('left, finished');
		expect(stoppedText(pickUp({ presence: 'running', last_event_at: '' }), NOW)).toBe('running');
	});
});

describe('sinceText', () => {
	it('counts hours up to two days, then days', () => {
		expect(sinceText(14 * HOUR)).toBe('14 h');
		expect(sinceText(47 * HOUR)).toBe('47 h');
		expect(sinceText(72 * HOUR)).toBe('3 d');
	});
});
