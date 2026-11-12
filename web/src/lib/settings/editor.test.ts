import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { PreferenceKey, SettingsSaved } from '#lib/api/settings.ts';
import { PreferenceEditor } from '#lib/settings/editor.svelte.ts';
import type * as preferences from '#lib/settings/preferences.ts';
import { DEFAULTS, save } from '#lib/settings/preferences.ts';
import { settingsState } from '#lib/testing/builders.ts';

vi.mock('$app/navigation', () => ({ refreshAll: vi.fn(() => Promise.resolve()) }));
vi.mock('#lib/settings/preferences.ts', async (original) => ({
	...(await original<typeof preferences>()),
	save: vi.fn()
}));

const state = settingsState(DEFAULTS);
const answer = (changed: PreferenceKey[], restart = false): SettingsSaved => ({
	...state,
	changed,
	restart_required: restart
});

beforeEach(() => vi.mocked(save).mockReset());

describe('PreferenceEditor', () => {
	it('shows the stored value until a change', () => {
		expect(new PreferenceEditor(() => state).value('week_start')).toBe('monday');
	});

	it('says whether a saved change applies now, needs a restart, or changed nothing', async () => {
		const editor = new PreferenceEditor(() => state);
		vi.mocked(save).mockResolvedValueOnce(answer(['week_start']));
		expect(await editor.set({ week_start: 'sunday' })).toBe(true);
		expect(editor.message).toBe('Saved. It applies right away.');
		vi.mocked(save).mockResolvedValueOnce(answer(['hide_secrets_on_ingest'], true));
		await editor.set({ hide_secrets_on_ingest: true });
		expect(editor.message).toBe('Saved. Restart GitPrompt for it to take effect.');
		vi.mocked(save).mockResolvedValueOnce(answer([]));
		await editor.set({ week_start: 'monday' });
		expect(editor.message).toBe('Nothing changed.');
	});

	it('shows the change while it saves, and the stored value again after a failure', async () => {
		const editor = new PreferenceEditor(() => state);
		let fail = (_: Error) => {};
		vi.mocked(save).mockReturnValueOnce(new Promise((_, reject) => (fail = reject)));
		const saving = editor.set({ week_start: 'sunday' });
		expect([editor.value('week_start'), editor.task.busy]).toEqual(['sunday', true]);
		fail(new Error('daemon down'));
		expect(await saving).toBe(false);
		expect([editor.value('week_start'), editor.task.error, editor.message]).toEqual([
			'monday',
			'daemon down',
			''
		]);
	});
});
