/**
 * The preferences other pages read (default range, week start), loaded once and shared until a
 * setting is saved. A daemon error falls back to the defaults: a page never fails over a setting.
 */
import { settingsApi, type Preferences, type SettingsState } from '#lib/api/settings.ts';
import { DEFAULT_RANGE } from '#lib/insights/range.ts';
import { forget } from '#lib/settings/remembered.ts';

type Fetch = typeof fetch;

export const DEFAULTS: Preferences = {
	default_range: DEFAULT_RANGE,
	week_start: 'monday',
	event_retention_days: null,
	hide_secrets_on_ingest: false,
	disabled_sources: []
};

/** The effective value of every preference. */
export function values(state: SettingsState): Preferences {
	const out = { ...DEFAULTS } as Record<string, unknown>;
	for (const [key, setting] of Object.entries(state.settings)) out[key] = setting.value;
	return out as Preferences;
}

let cache: Promise<Preferences> | null = null;

export function preferences(fetcher: Fetch = fetch): Promise<Preferences> {
	cache ??= settingsApi
		.get(fetcher)
		.then(values)
		.catch(() => {
			cache = null;
			return DEFAULTS;
		});
	return cache;
}

/**
 * Save some preferences and drop the shared copy, so every page reads the new values. A new
 * default range also drops the range remembered in this browser, so pages open with it.
 */
export async function save(changes: Parameters<typeof settingsApi.save>[0]) {
	try {
		const saved = await settingsApi.save(changes);
		if ('default_range' in changes) forget('range');
		return saved;
	} finally {
		cache = null;
	}
}
