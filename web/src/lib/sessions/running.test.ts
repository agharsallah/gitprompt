import { describe, expect, it } from 'vitest';
import {
	ageText,
	endedOutcome,
	endedPrompt,
	eventAge,
	presence,
	RUNNING_THRESHOLD_MS,
	runningLine
} from '#lib/sessions/running.ts';
import { prompt } from '#lib/testing/builders.ts';

const NOW = Date.parse('2026-11-05T10:00:00Z');
const ago = (ms: number) => new Date(NOW - ms).toISOString();

describe('presence', () => {
	it('is running while the last event is younger than the threshold', () => {
		expect(presence({ lastEventAt: ago(12_000) }, NOW, 'open')).toBe('running');
		expect(presence({ lastEventAt: ago(RUNNING_THRESHOLD_MS - 1) }, NOW, 'open')).toBe('running');
	});

	it('is left from the threshold on', () => {
		expect(presence({ lastEventAt: ago(RUNNING_THRESHOLD_MS) }, NOW, 'open')).toBe('left');
		expect(presence({ lastEventAt: ago(RUNNING_THRESHOLD_MS * 5) }, NOW, 'open')).toBe('left');
	});

	it('is left when the latest turn has an end, however fresh the last event', () => {
		expect(presence({ lastEventAt: ago(1000), ended: true }, NOW, 'open')).toBe('left');
		expect(presence({ lastEventAt: ago(1000), ended: false }, NOW, 'open')).toBe('running');
	});

	it('is left without a last event, or one it cannot read', () => {
		expect(presence({}, NOW, 'open')).toBe('left');
		expect(presence({ lastEventAt: null }, NOW, 'open')).toBe('left');
		expect(presence({ lastEventAt: 'yesterday-ish' }, NOW, 'open')).toBe('left');
	});

	it('claims running only while the stream can still bring news', () => {
		const fresh = { lastEventAt: ago(1000) };
		const states = (['open', 'connecting', 'reconnecting', 'closed'] as const).map((stream) =>
			presence(fresh, NOW, stream)
		);
		expect(states).toEqual(['running', 'running', 'left', 'left']);
	});

	it('takes an event a little ahead of the page clock as just now', () => {
		expect(eventAge(ago(-3000), NOW)).toBe(0);
		expect(presence({ lastEventAt: ago(-3000) }, NOW, 'open')).toBe('running');
	});
});

describe('the running line', () => {
	it('writes the age in seconds, minutes, then hours', () => {
		expect([0, 12_000, 59_999, 60_000, 185_000, 3_600_000, 7_300_000].map(ageText)).toEqual([
			'0 s',
			'12 s',
			'59 s',
			'1 min',
			'3 min',
			'1 h',
			'2 h'
		]);
	});

	it('is the sentence of the design', () => {
		expect(runningLine(12_000)).toBe('Running · last event 12 s ago');
	});
});

describe('when a turn is over', () => {
	it('is for the outcomes that have an end', () => {
		const ended = (['done', 'failed', 'interrupted', 'superseded', 'running', 'open'] as const).map(
			endedOutcome
		);
		expect(ended).toEqual([true, true, true, false, false, false]);
		expect(endedOutcome(undefined)).toBe(false);
	});

	it('is read from the latest prompt of a session', () => {
		const first = prompt({ summary: { ended_by: 'Stop' } });
		const second = prompt({ summary: {} });
		expect(endedPrompt([first])).toBe(true);
		expect(endedPrompt([first, second])).toBe(false);
		expect(endedPrompt([])).toBe(false);
	});
});
