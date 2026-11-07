import { refreshAll } from '$app/navigation';
import type { PreferenceKey, Preferences, SettingsState } from '#lib/api/settings.ts';
import { save } from '#lib/settings/preferences.ts';
import { Task } from '#lib/shared/task.svelte.ts';

type Changes = Partial<{ [K in PreferenceKey]: Preferences[K] | null }>;

/**
 * Saving preferences from one Settings section: a change shows at once (pending), is saved, then
 * the page reloads its data. On failure the control goes back to the stored value and the error shows.
 */
export class PreferenceEditor {
	readonly task = new Task();
	message = $state('');
	#pending = $state<Changes>({});
	#state: () => SettingsState;

	constructor(state: () => SettingsState) {
		this.#state = state;
	}

	value<K extends PreferenceKey>(key: K): Preferences[K] {
		return key in this.#pending
			? (this.#pending[key] as Preferences[K])
			: this.#state().settings[key].value;
	}

	async set(changes: Changes): Promise<boolean> {
		this.#pending = { ...this.#pending, ...changes };
		this.message = '';
		const ok = await this.task.run(async () => {
			const saved = await save(changes);
			await refreshAll();
			this.message = saved.restart_required
				? 'Saved. Restart GitPrompt for it to take effect.'
				: saved.changed.length
					? 'Saved. It applies right away.'
					: 'Nothing changed.';
		});
		this.#pending = {};
		return ok;
	}
}
