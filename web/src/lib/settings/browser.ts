/**
 * Preferences that live in this browser only (localStorage), listed in one place so the Settings
 * page can show and reset them all. To add one, add an entry here; `stored()` covers a plain
 * localStorage key, a key with its own reactive state (the theme) provides its own `current`/`reset`.
 */
import { theme } from '#lib/layout/theme.svelte.ts';

export type BrowserPreference = {
	key: string;
	label: string;
	/** Where it is changed. */
	where: string;
	/** The current value in words. */
	current: () => string;
	/** Back to the default. */
	reset: () => void;
};

/** localStorage can be missing or blocked (private windows): reads then return null. */
export function read(key: string): string | null {
	try {
		return localStorage.getItem(key);
	} catch {
		return null;
	}
}

export function forget(key: string): void {
	try {
		localStorage.removeItem(key);
	} catch {
		// nothing stored, nothing to forget
	}
}

/** An entry for a plain localStorage key: `describe(null)` says what the default is. */
export function stored(
	key: string,
	label: string,
	where: string,
	describe: (value: string | null) => string
): BrowserPreference {
	return { key, label, where, current: () => describe(read(key)), reset: () => forget(key) };
}

const THEMES = { system: 'Match system', light: 'Light', dark: 'Dark' } as const;

export const BROWSER_PREFERENCES: BrowserPreference[] = [
	{
		key: 'gitprompt-theme',
		label: 'Color theme',
		where: 'General, or the toggle at the bottom of the sidebar',
		current: () => THEMES[theme.value],
		reset: () => (theme.value = 'system')
	}
];
