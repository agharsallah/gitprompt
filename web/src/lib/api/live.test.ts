import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { live, liveUpdates, RETRY_MS } from '#lib/api/live.svelte.ts';
import { STREAM_URL } from '#lib/api/client.ts';

/** A stand-in for the browser's EventSource that the test drives by hand. */
class FakeSource {
	static readonly CONNECTING = 0;
	static readonly OPEN = 1;
	static readonly CLOSED = 2;
	static opened: FakeSource[] = [];
	readyState = FakeSource.CONNECTING;
	closed = false;
	onopen: (() => void) | null = null;
	onerror: (() => void) | null = null;
	onmessage: (() => void) | null = null;

	constructor(readonly url: string) {
		FakeSource.opened.push(this);
	}

	close() {
		this.closed = true;
		this.readyState = FakeSource.CLOSED;
	}

	/** The server answered with an error: the browser gives up on this source. */
	fail() {
		this.readyState = FakeSource.CLOSED;
		this.onerror?.();
	}
}

const latest = () => FakeSource.opened.at(-1);

beforeEach(() => {
	FakeSource.opened = [];
	vi.useFakeTimers();
	vi.stubGlobal('EventSource', FakeSource);
});
afterEach(() => {
	vi.useRealTimers();
	vi.unstubAllGlobals();
});

describe('live', () => {
	it('opens one stream while anything listens, and closes it after the last one leaves', () => {
		const first = live.subscribe(() => {});
		const second = live.subscribe(() => {});
		expect(FakeSource.opened.map((s) => s.url)).toEqual([STREAM_URL]);
		expect(live.state).toBe('connecting');
		latest()?.onopen?.();
		expect(live.state).toBe('open');
		first();
		expect(latest()?.closed).toBe(false);
		second();
		expect([latest()?.closed, live.state]).toEqual([true, 'closed']);
	});

	it('tells every listener about a notice', () => {
		const heard: string[] = [];
		const stop = [live.subscribe(() => heard.push('a')), live.subscribe(() => heard.push('b'))];
		latest()?.onmessage?.();
		expect(heard).toEqual(['a', 'b']);
		expect(live.lastNotice).not.toBeNull();
		stop.forEach((s) => s());
	});

	it('reports a network error as reconnecting and leaves the retry to the browser', () => {
		const stop = live.subscribe(() => {});
		latest()?.onerror?.();
		expect(live.state).toBe('reconnecting');
		vi.advanceTimersByTime(RETRY_MS);
		expect(FakeSource.opened).toHaveLength(1);
		stop();
	});

	it('opens a new stream after the server closed it, while someone still listens', () => {
		const stop = live.subscribe(() => {});
		latest()?.fail();
		expect(live.state).toBe('reconnecting');
		vi.advanceTimersByTime(RETRY_MS);
		expect(FakeSource.opened).toHaveLength(2);
		latest()?.onopen?.();
		expect(live.state).toBe('open');
		stop();
	});

	it('does not reopen once nobody listens', () => {
		const stop = live.subscribe(() => {});
		latest()?.fail();
		stop();
		vi.advanceTimersByTime(RETRY_MS);
		expect([FakeSource.opened.length, live.state]).toEqual([1, 'closed']);
	});
});

describe('liveUpdates', () => {
	it('calls back once a burst of notices settles', () => {
		const changed = vi.fn();
		const stop = liveUpdates(changed, 400);
		for (let i = 0; i < 5; i++) {
			latest()?.onmessage?.();
			vi.advanceTimersByTime(100);
		}
		expect(changed).not.toHaveBeenCalled();
		vi.advanceTimersByTime(400);
		expect(changed).toHaveBeenCalledTimes(1);
		latest()?.onmessage?.();
		stop();
		vi.advanceTimersByTime(400);
		expect(changed).toHaveBeenCalledTimes(1);
	});
});
