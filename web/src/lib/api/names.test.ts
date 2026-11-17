import { describe, expect, it } from 'vitest';
import { ApiError } from '#lib/api/client.ts';
import { namesApi } from '#lib/api/names.ts';
import { saveSessionName } from '#lib/sessions/rename.ts';
import { fakeFetch } from '#lib/testing/fetch.ts';

describe('names endpoints', () => {
	it('sets a name with a PUT that carries the header and the encoded id', async () => {
		const { fetch, calls } = fakeFetch(() => ({ body: { name: 'The parser' } }));
		expect(await namesApi.setSession('a b/c', 'The  parser', fetch)).toEqual({
			name: 'The parser'
		});
		expect(calls).toHaveLength(1);
		expect(calls[0]).toMatchObject({
			method: 'PUT',
			url: '/api/names/session/a%20b%2Fc',
			body: { name: 'The  parser' }
		});
		expect(calls[0].headers['x-gitprompt']).toBe('1');
	});

	it('forgets a name with a DELETE that carries the header', async () => {
		const { fetch, calls } = fakeFetch(() => ({ raw: '' }));
		await namesApi.clearSession('s1', fetch);
		expect(calls[0]).toMatchObject({ method: 'DELETE', url: '/api/names/session/s1' });
		expect(calls[0].headers['x-gitprompt']).toBe('1');
	});

	it('reports a refusal', async () => {
		const { fetch } = fakeFetch(() => ({ status: 422, statusText: 'Unprocessable', raw: '{}' }));
		await expect(namesApi.setSession('s1', 'x', fetch)).rejects.toBeInstanceOf(ApiError);
		await expect(namesApi.clearSession('s1', fetch)).rejects.toBeInstanceOf(ApiError);
	});
});

describe('saveSessionName', () => {
	it('answers the stored name of a rename', async () => {
		const { fetch } = fakeFetch(() => ({ body: { name: 'Stored' } }));
		expect(await saveSessionName('s1', 'stored', fetch)).toBe('Stored');
	});

	it('forgets the name and answers null for null', async () => {
		const { fetch, calls } = fakeFetch(() => ({ raw: '' }));
		expect(await saveSessionName('s1', null, fetch)).toBeNull();
		expect(calls.map((c) => c.method)).toEqual(['DELETE']);
	});
});
