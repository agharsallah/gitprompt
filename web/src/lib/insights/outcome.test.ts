import { describe, expect, it } from 'vitest';
import {
	ACTIVE_WINDOW_MS,
	outcome,
	RUNNING_WINDOW_MS,
	supersededIds
} from '#lib/insights/outcome.ts';
import { prompt } from '#lib/testing/builders.ts';

const sent = '2026-11-05T10:00:00.000Z';
const t0 = Date.parse(sent);

describe('outcome', () => {
	it('is done when the turn ended with Stop', () => {
		expect(outcome(prompt({ summary: { ended_by: 'Stop' } }))).toBe('done');
	});

	it('is failed when the turn ended with StopFailure', () => {
		expect(outcome(prompt({ summary: { ended_by: 'StopFailure' } }))).toBe('failed');
	});

	it('is running for a recent prompt without an end', () => {
		expect(outcome(prompt({ received_at: sent }), t0 + RUNNING_WINDOW_MS - 1)).toBe('running');
	});

	it('is open once the running window has passed', () => {
		expect(outcome(prompt({ received_at: sent }), t0 + RUNNING_WINDOW_MS + 1)).toBe('open');
	});

	it('is open when a later prompt of the session exists, however recent', () => {
		expect(outcome(prompt({ received_at: sent }), t0 + 1000, true)).toBe('open');
	});

	it('follows the last event when it is known: a long turn stays running while events arrive', () => {
		const now = t0 + 3 * RUNNING_WINDOW_MS;
		expect(outcome(prompt({ received_at: sent }), now, false, now - 1000)).toBe('running');
		expect(outcome(prompt({ received_at: sent }), now, false, now - ACTIVE_WINDOW_MS - 1)).toBe(
			'open'
		);
	});

	it('lets an end event win over everything else', () => {
		expect(outcome(prompt({ summary: { ended_by: 'Stop' } }), t0, true, t0)).toBe('done');
	});
});

describe('supersededIds', () => {
	it('marks every prompt but the latest of each session', () => {
		const a1 = prompt({ id: 1, fields: { session_id: 'a' } });
		const b1 = prompt({ id: 2, fields: { session_id: 'b' } });
		const a2 = prompt({ id: 3, fields: { session_id: 'a' } });
		expect(supersededIds([a2, b1, a1])).toEqual(new Set([1]));
	});

	it('ignores prompts without a session', () => {
		expect(supersededIds([prompt({ id: 9, fields: { session_id: undefined } })])).toEqual(
			new Set()
		);
	});
});
