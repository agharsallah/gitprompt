import { afterEach, describe, expect, it, vi } from 'vitest';
import { BROWSER_PREFERENCES, read, stored } from '#lib/settings/browser.ts';
import { memoryStorage } from '#lib/testing/storage.ts';

afterEach(() => vi.unstubAllGlobals());

describe('browser-only preferences', () => {
	it('reads, describes and resets a plain localStorage key', () => {
		vi.stubGlobal('localStorage', memoryStorage());
		const pref = stored('gitprompt-x', 'X', 'Prompts', (v) => v ?? 'default');
		expect(pref.current()).toBe('default');
		localStorage.setItem('gitprompt-x', 'typed');
		expect(pref.current()).toBe('typed');
		pref.reset();
		expect(read('gitprompt-x')).toBeNull();
	});

	it('treats blocked storage as nothing stored', () => {
		vi.stubGlobal('localStorage', {
			getItem: () => {
				throw new Error('blocked');
			},
			removeItem: () => {
				throw new Error('blocked');
			}
		});
		const pref = stored('gitprompt-x', 'X', 'Prompts', (v) => v ?? 'default');
		expect(pref.current()).toBe('default');
		expect(() => pref.reset()).not.toThrow();
	});

	it('lists every key under the gitprompt- prefix once', () => {
		const keys = BROWSER_PREFERENCES.map((p) => p.key);
		expect(new Set(keys).size).toBe(keys.length);
		expect(keys.every((k) => k.startsWith('gitprompt-'))).toBe(true);
	});
});

describe('each listed preference', () => {
	const pref = (key: string) => {
		const found = BROWSER_PREFERENCES.find((p) => p.key === key);
		if (!found) throw new Error(`no preference ${key}`);
		return found;
	};

	it('describes its value and resets it', () => {
		vi.stubGlobal('localStorage', memoryStorage());
		vi.stubGlobal('document', { documentElement: { dataset: {} } });
		for (const p of BROWSER_PREFERENCES) {
			expect(typeof p.current()).toBe('string');
			expect(() => p.reset()).not.toThrow();
		}
		expect(pref('gitprompt-theme').current()).toBe('Match system');
		expect(pref('gitprompt-update-hint').current()).toBe('Shown');
		localStorage.setItem('gitprompt-update-hint', 'x');
		expect(pref('gitprompt-update-hint').current()).toBe(
			'Hidden until another connection needs an update'
		);
		pref('gitprompt-update-hint').reset();
		expect(read('gitprompt-update-hint')).toBeNull();
	});
});
