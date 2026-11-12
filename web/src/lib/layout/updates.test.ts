import { afterEach, describe, expect, it, vi } from 'vitest';
import { updates } from '#lib/layout/updates.svelte.ts';
import { DISMISS_KEY } from '#lib/sources/updates.ts';
import { fakeFetch } from '#lib/testing/fetch.ts';
import { memoryStorage } from '#lib/testing/storage.ts';

afterEach(() => vi.unstubAllGlobals());

/** Lets the refresh's promise chain settle. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

describe('updates', () => {
	it('keeps no hint when the daemon cannot be reached', async () => {
		vi.stubGlobal('fetch', fakeFetch(() => ({ status: 500 })).fetch);
		updates.refresh();
		await settle();
		expect(updates.list).toEqual([]);
	});

	it('remembers "Later" in this browser, and forgets it on reset', () => {
		const storage = memoryStorage();
		vi.stubGlobal('localStorage', storage);
		updates.dismiss();
		expect(storage.getItem(DISMISS_KEY)).not.toBeNull();
		updates.reset();
		expect(storage.getItem(DISMISS_KEY)).toBeNull();
	});
});
