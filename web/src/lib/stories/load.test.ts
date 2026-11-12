import { isHttpError } from '@sveltejs/kit';
import { describe, expect, it } from 'vitest';
import { ApiError } from '#lib/api/client.ts';
import { orNotFound } from '#lib/stories/load.ts';

describe('orNotFound', () => {
	it('passes the story through', async () => {
		expect(await orNotFound(Promise.resolve({ id: 's' }), 'session')).toEqual({ id: 's' });
	});

	it('turns a story the daemon does not know into a 404 page', async () => {
		const missing = Promise.reject(new ApiError(404, '/api/stories/session/x', 'not found'));
		const error: unknown = await orNotFound(missing, 'session').catch((e: unknown) => e);
		expect(isHttpError(error, 404)).toBe(true);
		expect(isHttpError(error) && error.body.message).toBe('No session with this id was captured.');
	});

	it('lets every other failure through as it is', async () => {
		const down = new ApiError(500, '/api/stories/project/x', 'boom');
		await expect(orNotFound(Promise.reject(down), 'project')).rejects.toBe(down);
		const offline = new TypeError('fetch failed');
		await expect(orNotFound(Promise.reject(offline), 'project')).rejects.toBe(offline);
	});
});
