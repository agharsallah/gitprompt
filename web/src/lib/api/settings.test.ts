import { afterEach, describe, expect, it, vi } from 'vitest';
import { settingsApi } from '#lib/api/settings.ts';
import { fakeFetch } from '#lib/testing/fetch.ts';

afterEach(() => vi.unstubAllGlobals());

describe('settings endpoints', () => {
	it('reads settings, data and security', async () => {
		const { fetch, calls } = fakeFetch();
		await settingsApi.get(fetch);
		await settingsApi.data(fetch);
		await settingsApi.security(fetch);
		expect(calls.map((c) => c.url)).toEqual(['/api/settings', '/api/data', '/api/security']);
	});

	it('previews every destructive action unless told to apply', async () => {
		const { fetch, calls } = fakeFetch();
		vi.stubGlobal('fetch', fetch);
		await settingsApi.compact(30);
		await settingsApi.compact(null, false);
		await settingsApi.deleteEvents({ source: 'codex', before: null });
		await settingsApi.deleteEvents({ source: 'codex', before: null }, false);
		await settingsApi.rotateToken();
		await settingsApi.rotateToken(false);
		expect(calls.map((c) => [c.method, c.url, c.body])).toEqual([
			['POST', '/api/data/compact', { older_than_days: 30 }],
			['POST', '/api/data/compact?dry_run=false', { older_than_days: null }],
			['POST', '/api/data/delete-events', { source: 'codex', before: null }],
			['POST', '/api/data/delete-events?dry_run=false', { source: 'codex', before: null }],
			['POST', '/api/security/rotate-token', undefined],
			['POST', '/api/security/rotate-token?dry_run=false', undefined]
		]);
	});

	it('saves only the changed settings, null meaning back to the default', async () => {
		const { fetch, calls } = fakeFetch();
		vi.stubGlobal('fetch', fetch);
		await settingsApi.save({ week_start: 'sunday', event_retention_days: null });
		expect(calls[0]).toMatchObject({
			method: 'PUT',
			url: '/api/settings',
			body: { week_start: 'sunday', event_retention_days: null }
		});
	});
});
