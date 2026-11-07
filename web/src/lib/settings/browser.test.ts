import { afterEach, describe, expect, it, vi } from 'vitest';
import { BROWSER_PREFERENCES, read, stored } from '#lib/settings/browser.ts';

function memoryStorage() {
	const items = new Map<string, string>();
	return {
		getItem: (k: string) => items.get(k) ?? null,
		setItem: (k: string, v: string) => void items.set(k, v),
		removeItem: (k: string) => void items.delete(k)
	};
}

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
