import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { presence, RUNNING_THRESHOLD_MS } from '#lib/sessions/running.ts';

// The shared clock only ticks in a browser: give it a window and a visible document, on fake
// timers, and load it fresh so its interval is the fake one.
const START = Date.parse('2026-11-05T10:00:00Z');

beforeEach(() => {
	vi.useFakeTimers();
	vi.setSystemTime(START);
	vi.resetModules();
	vi.stubGlobal('window', {});
	vi.stubGlobal('document', { visibilityState: 'visible', addEventListener: () => {} });
});
afterEach(() => {
	vi.useRealTimers();
	vi.unstubAllGlobals();
});

async function sharedClock() {
	return (await import('#lib/shared/clock.svelte.ts')).clock;
}

describe('a session on the shared clock', () => {
	it('flips from running to left once its last event is old enough, with no event to say so', async () => {
		const clock = await sharedClock();
		const activity = { lastEventAt: new Date(START - 12_000).toISOString() };
		const state = () => presence(activity, clock.now, 'open');
		expect(state()).toBe('running');

		vi.advanceTimersByTime(RUNNING_THRESHOLD_MS / 2);
		expect(state()).toBe('running');

		vi.advanceTimersByTime(RUNNING_THRESHOLD_MS / 2 + 15_000);
		expect(state()).toBe('left');
	});

	it('stays left, whatever the clock does, once the latest turn has ended', async () => {
		const clock = await sharedClock();
		const activity = { lastEventAt: new Date(START).toISOString(), ended: true };
		vi.advanceTimersByTime(15_000);
		expect(presence(activity, clock.now, 'open')).toBe('left');
	});
});
