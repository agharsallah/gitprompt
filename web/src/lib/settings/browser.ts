/**
 * Preferences that live in this browser only (localStorage), listed in one place so the Settings
 * page can show and reset them all. To add one, add an entry here; `stored()` covers a plain
 * localStorage key, a key with its own reactive state (the theme) provides its own `current`/`reset`.
 */
import { theme } from '#lib/layout/theme.svelte.ts';
import { updates } from '#lib/layout/updates.svelte.ts';
import { DISMISS_KEY } from '#lib/sources/updates.ts';
import { ORIGINS } from '#lib/prompts/origin.ts';
import { isAll, originFilter } from '#lib/prompts/originFilter.svelte.ts';
import { describeRange, describeView, RANGE_KEY, VIEW_PAGES, VIEWS } from '#lib/settings/views.ts';
import { views } from '#lib/settings/views.svelte.ts';

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
	},
	{
		key: 'gitprompt-origins',
		label: 'Prompts shown by origin',
		where: 'The origin filter on Prompts',
		current: () =>
			isAll(originFilter.origins)
				? 'Every origin'
				: originFilter.origins.map((o) => ORIGINS[o].label).join(', '),
		reset: () => originFilter.reset()
	},
	{
		key: DISMISS_KEY,
		label: 'Hidden update hint',
		where: '“Later” on the update hint in the sidebar',
		current: () =>
			read(DISMISS_KEY) ? 'Hidden until another connection needs an update' : 'Shown',
		reset: () => updates.reset()
	},
	{
		key: RANGE_KEY,
		label: 'Time range',
		where: 'The range picker, the same on every page',
		current: describeRange,
		reset: () => views.forgetRange()
	},
	...VIEW_PAGES.map((page) => ({
		key: VIEWS[page].key,
		label: `Filters on ${VIEWS[page].label}`,
		where: `The filters on ${VIEWS[page].label}`,
		current: () => describeView(page),
		reset: () => views.forget(page)
	}))
];
