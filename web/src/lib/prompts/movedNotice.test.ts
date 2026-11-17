import { describe, expect, it } from 'vitest';
import {
	cameFromSearch,
	markSeen,
	MOVED_KEY,
	shouldShow,
	wasSeen
} from '#lib/prompts/movedNotice.ts';
import { blockedStorage, memoryStorage } from '#lib/testing/storage.ts';

const fromSearch = new URLSearchParams('range=7d&q=a&from=search');
const plain = new URLSearchParams('range=7d&q=a');

describe('the "Search now lives here" line', () => {
	it('is for visits that came from the old Search page only', () => {
		expect(cameFromSearch(fromSearch)).toBe(true);
		expect(cameFromSearch(plain)).toBe(false);
		expect(cameFromSearch(new URLSearchParams('from=elsewhere'))).toBe(false);
	});

	it('shows once: not again after it was seen in this browser', () => {
		const storage = memoryStorage();
		expect(shouldShow(fromSearch, storage)).toBe(true);
		markSeen(storage);
		expect(storage.items.get(MOVED_KEY)).toBe('1');
		expect(wasSeen(storage)).toBe(true);
		expect(shouldShow(fromSearch, storage)).toBe(false);
	});

	it('does not show on a visit that did not come from Search', () => {
		expect(shouldShow(plain, memoryStorage())).toBe(false);
	});

	it('shows, and breaks nothing, when storage is blocked', () => {
		const storage = blockedStorage();
		expect(shouldShow(fromSearch, storage)).toBe(true);
		expect(() => markSeen(storage)).not.toThrow();
	});
});
