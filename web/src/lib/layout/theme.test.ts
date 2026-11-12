import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { blockedStorage, memoryStorage } from '#lib/testing/storage.ts';

/** A fresh copy of the module, reading the storage and document stubbed for this test. */
async function load() {
	vi.resetModules();
	return import('#lib/layout/theme.svelte.ts');
}

let root: { dataset: Record<string, string> };

beforeEach(() => {
	root = { dataset: {} };
	vi.stubGlobal('document', { documentElement: root });
});
afterEach(() => vi.unstubAllGlobals());

describe('theme', () => {
	it('starts from the stored theme, or the system one', async () => {
		vi.stubGlobal('localStorage', memoryStorage({ 'gitprompt-theme': 'dark' }));
		expect((await load()).theme.value).toBe('dark');
		vi.stubGlobal('localStorage', memoryStorage({ 'gitprompt-theme': 'sepia' }));
		expect((await load()).theme.value).toBe('system');
		vi.stubGlobal('localStorage', blockedStorage());
		expect((await load()).theme.value).toBe('system');
	});

	it('pins a chosen theme on <html> and in storage, and unpins the system one', async () => {
		const storage = memoryStorage();
		vi.stubGlobal('localStorage', storage);
		const { theme, THEME_KEY } = await load();
		theme.value = 'light';
		expect([root.dataset.theme, storage.getItem(THEME_KEY)]).toEqual(['light', 'light']);
		theme.value = 'system';
		expect([root.dataset.theme, storage.getItem(THEME_KEY)]).toEqual([undefined, null]);
	});

	it('still switches for this visit when storage is blocked', async () => {
		vi.stubGlobal('localStorage', blockedStorage());
		const { theme } = await load();
		theme.value = 'dark';
		expect([theme.value, root.dataset.theme]).toEqual(['dark', 'dark']);
	});
});
