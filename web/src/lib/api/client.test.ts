import { afterEach, describe, expect, it, vi } from 'vitest';
import { api, ApiError, get, write } from '#lib/api/client.ts';
import { fakeFetch } from '#lib/testing/fetch.ts';

afterEach(() => vi.unstubAllGlobals());

/** The ApiError a request rejects with. */
async function failure(request: Promise<unknown>): Promise<ApiError> {
	const error = await request.then(
		() => null,
		(e: unknown) => e
	);
	if (!(error instanceof ApiError)) throw new Error(`expected an ApiError, got ${String(error)}`);
	return error;
}

describe('reading', () => {
	it('returns the JSON of a successful answer', async () => {
		const { fetch, calls } = fakeFetch(() => ({ body: [{ id: 1 }] }));
		expect(await get('/api/x', fetch)).toEqual([{ id: 1 }]);
		expect(calls).toEqual([{ url: '/api/x', method: 'GET', headers: {}, body: undefined }]);
	});

	it('puts the status, the path and the daemon’s detail in the error', async () => {
		const { fetch } = fakeFetch(() => ({ status: 404, body: { detail: 'prompt not found' } }));
		const error = await failure(get('/api/prompts/9', fetch));
		expect([error.status, error.path, error.message]).toEqual([
			404,
			'/api/prompts/9',
			'404 /api/prompts/9: prompt not found'
		]);
		expect(error.name).toBe('ApiError');
	});

	it('joins validation errors, with or without a location', async () => {
		const detail = [{ loc: ['query', 'limit'], msg: 'too big' }, { msg: 'bad range' }, 'plain'];
		const { fetch } = fakeFetch(() => ({ status: 422, body: { detail } }));
		expect((await failure(get('/api/x', fetch))).message).toBe(
			'422 /api/x: query.limit: too big; bad range; plain'
		);
	});

	it('falls back to the status text when the body says nothing usable', async () => {
		for (const reply of [
			{ raw: 'not json' },
			{ body: { detail: { nested: true } } },
			{ body: { detail: '' } }
		]) {
			const { fetch } = fakeFetch(() => ({
				status: 500,
				statusText: 'Internal Server Error',
				...reply
			}));
			expect((await failure(get('/api/x', fetch))).message).toBe(
				'500 /api/x: Internal Server Error'
			);
		}
		const { fetch } = fakeFetch(() => ({ status: 503, raw: '' }));
		expect((await failure(get('/api/x', fetch))).message).toBe('503 /api/x: HTTP 503');
	});
});

describe('writing', () => {
	it('sends the cross-site header and a JSON body', async () => {
		const { fetch, calls } = fakeFetch(() => ({ body: { ok: true } }));
		vi.stubGlobal('fetch', fetch);
		expect(await write('/api/x', 'PUT', { a: 1 })).toEqual({ ok: true });
		expect(calls[0]).toEqual({
			url: '/api/x',
			method: 'PUT',
			headers: { 'x-gitprompt': '1', 'content-type': 'application/json' },
			body: { a: 1 }
		});
		await write('/api/y', 'POST');
		expect(calls[1].body).toBeUndefined();
	});
});

describe('endpoints', () => {
	it('builds each URL, escaping ids', async () => {
		const { fetch, calls } = fakeFetch(() => ({ body: {} }));
		vi.stubGlobal('fetch', fetch);
		await api.prompt(7);
		await api.annotate(7, { starred: true, rating: 0, tags: [], note: '' });
		await api.redact(7);
		await api.stats({ source: 'codex' });
		await api.telemetry();
		await api.sessionStory('a/b c', fetch);
		await api.projectStory('/w/app', fetch);
		await api.lineage(3, fetch);
		await api.search({ q: 'fix' }, fetch);
		await api.prompts(new URLSearchParams({ limit: '5' }), fetch);
		await api.sources(fetch);
		await api.configure('claude-code', 'connect', true);
		await api.configure('claude-code', 'disconnect');
		await api.remap('codex');
		await api.specProblems(fetch);
		await api.recentCaptures(fetch);
		await api.spec('cursor');
		await api.setEnabled('cursor', false);
		await api.setEnabled('cursor', true);
		await api.fields(fetch);
		expect(calls.map((c) => `${c.method} ${c.url}`)).toEqual([
			'GET /api/prompts/7',
			'PUT /api/prompts/7/annotation',
			'POST /api/prompts/7/redact',
			'GET /api/stats?source=codex',
			'GET /api/stats/telemetry?',
			'GET /api/stories/session/a%2Fb%20c',
			'GET /api/stories/project/%2Fw%2Fapp',
			'GET /api/stories/prompt/3',
			'GET /api/search?q=fix',
			'GET /api/prompts?limit=5',
			'GET /api/sources',
			'POST /api/sources/claude-code/connect?dry_run=true',
			'POST /api/sources/claude-code/disconnect',
			'POST /api/sources/codex/remap',
			'GET /api/sources/problems',
			'GET /api/sources/recent',
			'GET /api/sources/cursor/spec',
			'POST /api/sources/cursor/disable',
			'POST /api/sources/cursor/enable',
			'GET /api/fields'
		]);
	});
});
