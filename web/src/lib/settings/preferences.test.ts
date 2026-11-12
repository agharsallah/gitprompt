import { afterEach, describe, expect, it, vi } from 'vitest';
import { DEFAULTS, preferences, save, values } from '#lib/settings/preferences.ts';
import { views } from '#lib/settings/views.svelte.ts';
import { fakeFetch } from '#lib/testing/fetch.ts';
import { settingsState } from '#lib/testing/builders.ts';

afterEach(() => {
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});

const stored = { ...DEFAULTS, week_start: 'sunday' as const, event_retention_days: 30 };
const saved = { ...settingsState(stored), changed: ['week_start'], restart_required: false };

describe('preferences', () => {
	it('reads every effective value from the settings', () => {
		expect(values(settingsState(stored))).toEqual(stored);
	});

	it('falls back to the defaults when the daemon fails, then asks again next time', async () => {
		const down = fakeFetch(() => ({ status: 500 }));
		expect(await preferences(down.fetch)).toEqual(DEFAULTS);
		const up = fakeFetch(() => ({ body: settingsState(stored) }));
		expect(await preferences(up.fetch)).toEqual(stored);
		expect(await preferences(up.fetch)).toEqual(stored);
		expect(up.calls).toHaveLength(1);
	});

	it('reads them again after a save, and forgets the kept range only for a new default range', async () => {
		const forgetRange = vi.spyOn(views, 'forgetRange');
		vi.stubGlobal('fetch', fakeFetch(() => ({ body: saved })).fetch);
		await save({ week_start: 'sunday' });
		expect(forgetRange).not.toHaveBeenCalled();
		await save({ default_range: '7d' });
		expect(forgetRange).toHaveBeenCalledTimes(1);
		const after = fakeFetch(() => ({ body: settingsState(stored) }));
		await preferences(after.fetch);
		expect(after.calls).toHaveLength(1);
	});

	it('drops the shared copy even when the save fails', async () => {
		const first = fakeFetch(() => ({ body: settingsState(stored) }));
		await preferences(first.fetch);
		vi.stubGlobal('fetch', fakeFetch(() => ({ status: 422, body: { detail: 'bad value' } })).fetch);
		await expect(save({ week_start: 'sunday' })).rejects.toThrow('bad value');
		const again = fakeFetch(() => ({ body: settingsState(DEFAULTS) }));
		expect(await preferences(again.fetch)).toEqual(DEFAULTS);
	});
});
