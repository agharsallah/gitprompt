import { api } from '#lib/api/client.ts';
import { settingsApi, type SettingsState } from '#lib/api/settings.ts';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch }) => {
	// An invalid preferences.json must not hide the rest of the page: report it instead.
	const prefs: Promise<SettingsState | { error: string }> = settingsApi
		.get(fetch)
		.catch((e: Error) => ({ error: e.message }));
	const [settings, data, security, sources, problems] = await Promise.all([
		prefs,
		settingsApi.data(fetch),
		settingsApi.security(fetch),
		api.sources(fetch),
		api.specProblems(fetch)
	]);
	return { settings, data, security, sources, problems };
};
