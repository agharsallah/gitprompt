import { api } from '#lib/api/client.ts';
import { settingsApi, type SettingsState } from '#lib/api/settings.ts';
import { defer } from '#lib/async/resource.svelte.ts';
import type { PageLoad } from './$types';

async function everything(fetcher: typeof fetch) {
	// An invalid preferences.json must not hide the rest of the page: report it instead.
	const prefs: Promise<SettingsState | { error: string }> = settingsApi
		.get(fetcher)
		.catch((e: Error) => ({ error: e.message }));
	const [settings, data, security, sources, problems] = await Promise.all([
		prefs,
		settingsApi.data(fetcher),
		settingsApi.security(fetcher),
		api.sources(fetcher),
		api.specProblems(fetcher)
	]);
	return { settings, data, security, sources, problems };
}

// The page and its table of contents show at once; the settings stream in.
export const load: PageLoad = ({ fetch }) => ({ content: defer(everything(fetch)) });
