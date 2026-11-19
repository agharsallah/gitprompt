import { describe, expect, it } from 'vitest';
import type { Recap } from '#lib/api/types.ts';
import { dismissRecap, firstShowing, RECAP_SEEN_KEY } from '#lib/overview/recap.ts';
import { blockedStorage, memoryStorage } from '#lib/testing/storage.ts';

const recap = (due: string): Recap => ({
	due,
	notification: {
		kind: 'weekly_recap',
		title: 'Your week',
		body: 'This week: $42.00 · 1 h 10 min redone · 23 commits.',
		target: { page: 'overview', range: '7d' }
	}
});

const THIS_WEEK = recap('2026-11-16T09:00:00.000000+00:00');
const NEXT_WEEK = recap('2026-11-23T09:00:00.000000+00:00');

describe('the weekly recap card', () => {
	it('is shown the first time the Overview opens in the week, not on the next visit', () => {
		const storage = memoryStorage();
		expect(firstShowing(THIS_WEEK, storage)).toBe(true);
		expect(firstShowing(THIS_WEEK, storage)).toBe(false);
	});

	it("shows again with the next week's recap", () => {
		const storage = memoryStorage();
		firstShowing(THIS_WEEK, storage);
		expect(firstShowing(NEXT_WEEK, storage)).toBe(true);
	});

	it('is not shown without a recap this week', () => {
		const storage = memoryStorage();
		expect(firstShowing(null, storage)).toBe(false);
		expect(firstShowing(undefined, storage)).toBe(false);
		expect(storage.writes()).toBe(0);
	});

	it('a dismissed recap stays hidden for its week', () => {
		const storage = memoryStorage();
		dismissRecap(THIS_WEEK, storage);
		expect(storage.items.get(RECAP_SEEN_KEY)).toBe(THIS_WEEK.due);
		expect(firstShowing(THIS_WEEK, storage)).toBe(false);
	});

	it('with blocked storage it shows, and dismissing does not throw', () => {
		const storage = blockedStorage();
		expect(firstShowing(THIS_WEEK, storage)).toBe(true);
		expect(() => dismissRecap(THIS_WEEK, storage)).not.toThrow();
	});
});
