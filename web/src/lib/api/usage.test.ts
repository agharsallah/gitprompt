import { describe, expect, it } from 'vitest';
import { ApiError } from '#lib/api/client.ts';
import { usageApi } from '#lib/api/usage.ts';
import { fakeFetch } from '#lib/testing/fetch.ts';

describe('usage endpoints', () => {
	it('reads the counts of the last days from the relative path', async () => {
		const { fetch, calls } = fakeFetch(() => ({ body: { since: 'a', until: 'b', counts: [] } }));
		expect(await usageApi.get(30, fetch)).toEqual({ since: 'a', until: 'b', counts: [] });
		expect(calls.map((c) => [c.method, c.url])).toEqual([['GET', '/api/usage?days=30']]);
	});

	it('clears them with a DELETE that carries the dashboard header', async () => {
		const { fetch, calls } = fakeFetch(() => ({ raw: '' }));
		await usageApi.clear(fetch);
		expect(calls).toHaveLength(1);
		expect(calls[0]).toMatchObject({ method: 'DELETE', url: '/api/usage' });
		expect(calls[0].headers['x-gitprompt']).toBe('1');
	});

	it('reports a refusal', async () => {
		const { fetch } = fakeFetch(() => ({ status: 403, statusText: 'Forbidden', raw: '' }));
		await expect(usageApi.clear(fetch)).rejects.toBeInstanceOf(ApiError);
	});
});
