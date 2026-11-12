import { error } from '@sveltejs/kit';
import { describe, expect, it } from 'vitest';
import { ApiError } from '#lib/api/client.ts';
import { describeError, messageOf } from './describe.ts';

/** The HttpError that SvelteKit's error() throws. */
function httpError(status: number, message: string): unknown {
	try {
		error(status, message);
	} catch (e) {
		return e;
	}
}

describe('describeError', () => {
	it('reads a 404 from error() as a missing page, with its own message', () => {
		const d = describeError(httpError(404, 'No session with this id was captured.'));
		expect(d).toMatchObject({
			kind: 'not-found',
			status: 404,
			title: 'No session with this id was captured.',
			retry: false
		});
	});

	it('gives an unknown route a plain title', () => {
		const d = describeError({ message: 'Not Found' }, 404);
		expect(d.kind).toBe('not-found');
		expect(d.title).toBe('This page does not exist');
	});

	it('reads a failed fetch as the daemon being down', () => {
		const d = describeError(new TypeError('Failed to fetch'));
		expect(d).toMatchObject({ kind: 'offline', retry: true, reload: false });
	});

	it('reads the same failure once SvelteKit made it page.error', () => {
		expect(describeError({ message: 'Failed to fetch' }, 500).kind).toBe('offline');
	});

	it('does not treat any Error that mentions a network as offline', () => {
		expect(describeError(new Error('NetworkError in my code')).kind).toBe('unexpected');
	});

	it('asks for a reload when a lazily loaded chunk is gone', () => {
		const d = describeError(
			new TypeError('Failed to fetch dynamically imported module: /_app/immutable/x.js')
		);
		expect(d).toMatchObject({ kind: 'outdated', retry: false, reload: true });
	});

	it('names what failed when the daemon answers with a server error', () => {
		const d = describeError(new ApiError(500, '/api/stats', 'boom'), undefined, 'the overview');
		expect(d).toMatchObject({ kind: 'daemon', status: 500 });
		expect(d.title).toBe('The daemon could not load the overview');
		expect(d.detail).toBe('500 /api/stats: boom');
	});

	it('uses the page status for an error that lost its class (page.error)', () => {
		expect(describeError({ message: '502 /api/stats: Bad Gateway' }, 502).kind).toBe('daemon');
	});

	it('falls back to a generic, retryable error', () => {
		const d = describeError(new RangeError('bad'), undefined, 'this panel');
		expect(d).toMatchObject({ kind: 'unexpected', retry: true, detail: 'bad' });
		expect(d.title).toBe('Couldn’t show this panel');
	});
});

describe('messageOf', () => {
	it('reads errors, error-like objects and plain values', () => {
		expect(messageOf(new Error('a'))).toBe('a');
		expect(messageOf({ message: 'b' })).toBe('b');
		expect(messageOf('c')).toBe('c');
		expect(messageOf(undefined)).toBe('');
	});

	it('never says "[object Object]"', () => {
		expect(messageOf(42)).toBe('42');
		expect(messageOf({ message: { code: 7 } })).toBe('{"code":7}');
		expect(messageOf({ status: 'down' })).toBe('{"status":"down"}');
		const circular: Record<string, unknown> = {};
		circular.self = circular;
		expect(messageOf(circular)).toBe('');
	});
});
