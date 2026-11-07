/**
 * The color theme, a browser-only preference. One shared state, so the sidebar toggle and the
 * Settings page always agree. The pinned theme lives on <html data-theme>; app.html applies it
 * before first paint, and lib/settings/browser.ts lists the key on the Settings page.
 */
export type Theme = 'system' | 'light' | 'dark';
export const THEME_KEY = 'gitprompt-theme';

function stored(): Theme {
	try {
		const value = localStorage.getItem(THEME_KEY);
		return value === 'light' || value === 'dark' ? value : 'system';
	} catch {
		return 'system';
	}
}

function apply(theme: Theme) {
	const root = document.documentElement;
	if (theme === 'system') delete root.dataset.theme;
	else root.dataset.theme = theme;
	try {
		if (theme === 'system') localStorage.removeItem(THEME_KEY);
		else localStorage.setItem(THEME_KEY, theme);
	} catch {
		// Storage can be unavailable (private windows); the choice then lasts for this visit.
	}
}

class ThemeState {
	#value = $state<Theme>(stored());

	get value(): Theme {
		return this.#value;
	}

	set value(theme: Theme) {
		this.#value = theme;
		apply(theme);
	}
}

export const theme = new ThemeState();
