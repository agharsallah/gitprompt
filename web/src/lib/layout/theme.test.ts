import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { THEME_KEY, nextTheme, storedTheme, theme } from '#lib/layout/theme.svelte.ts';
import { blockedStorage, memoryStorage } from '#lib/testing/storage.ts';

// One import of the module for the whole file: re-importing it (vi.resetModules) evaluates the
// Svelte runtime again each time, seconds on a busy machine. The start value is storedTheme().

let root: { dataset: Record<string, string> };

beforeEach(() => {
	root = { dataset: {} };
	vi.stubGlobal('document', { documentElement: root });
});
afterEach(() => vi.unstubAllGlobals());

describe('theme', () => {
	it('starts from the stored theme, or the system one', () => {
		vi.stubGlobal('localStorage', memoryStorage({ [THEME_KEY]: 'dark' }));
		expect(storedTheme()).toBe('dark');
		vi.stubGlobal('localStorage', memoryStorage({ [THEME_KEY]: 'sepia' }));
		expect(storedTheme()).toBe('system');
		vi.stubGlobal('localStorage', blockedStorage());
		expect(storedTheme()).toBe('system');
	});

	it('pins a chosen theme on <html> and in storage, and unpins the system one', () => {
		const storage = memoryStorage();
		vi.stubGlobal('localStorage', storage);
		theme.value = 'light';
		expect([root.dataset.theme, storage.getItem(THEME_KEY)]).toEqual(['light', 'light']);
		theme.value = 'system';
		expect([root.dataset.theme, storage.getItem(THEME_KEY)]).toEqual([undefined, null]);
	});

	it('still switches for this visit when storage is blocked', () => {
		vi.stubGlobal('localStorage', blockedStorage());
		theme.value = 'dark';
		expect([theme.value, root.dataset.theme]).toEqual(['dark', 'dark']);
	});
});

describe('nextTheme', () => {
	it('switches to the opposite of the theme in effect', () => {
		expect(nextTheme('dark', false)).toBe('light');
		expect(nextTheme('light', true)).toBe('dark');
	});

	it('reads the system preference while the theme follows the system', () => {
		expect(nextTheme('system', true)).toBe('light');
		expect(nextTheme('system', false)).toBe('dark');
	});
});
