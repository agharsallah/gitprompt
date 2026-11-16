import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { UsageIn } from '#lib/api/types.ts';
import { UsageTracker, USAGE_URL, flushOnLeave, postUsage, track } from '#lib/shared/usage.ts';
import { fakeFetch } from '#lib/testing/fetch.ts';

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
	vi.useRealTimers();
	vi.unstubAllGlobals();
});

describe('UsageTracker', () => {
	it('sends nothing at once, then one report with every action counted, after the delay', () => {
		const sent: UsageIn[] = [];
		const tracker = new UsageTracker((report) => sent.push(report), 500);
		tracker.track('copy.prompt');
		tracker.track('export.open');
		tracker.track('copy.prompt');
		vi.advanceTimersByTime(499);
		expect(sent).toEqual([]);
		vi.advanceTimersByTime(1);
		expect(sent).toEqual([
			{
				hits: [
					{ action: 'copy.prompt', n: 2 },
					{ action: 'export.open', n: 1 }
				]
			}
		]);
	});

	it('starts a new batch after a send, and sends nothing when nothing was counted', () => {
		const sent: UsageIn[] = [];
		const tracker = new UsageTracker((report) => sent.push(report), 100);
		tracker.flush();
		expect(sent).toEqual([]);
		tracker.track('export.open');
		vi.advanceTimersByTime(100);
		tracker.track('export.open');
		vi.advanceTimersByTime(100);
		expect(sent.flatMap((report) => report.hits)).toEqual([
			{ action: 'export.open', n: 1 },
			{ action: 'export.open', n: 1 }
		]);
	});

	it('flushes on demand and does not send the same counts again later', () => {
		const sent: UsageIn[] = [];
		const tracker = new UsageTracker((report) => sent.push(report), 100);
		tracker.track('export.open');
		tracker.flush();
		vi.advanceTimersByTime(1000);
		expect(sent).toHaveLength(1);
	});

	it('flushes when one count reaches what a report may carry', () => {
		const sent: UsageIn[] = [];
		const tracker = new UsageTracker((report) => sent.push(report), 100);
		for (let i = 0; i < 1000; i++) tracker.track('export.open');
		expect(sent).toEqual([{ hits: [{ action: 'export.open', n: 1000 }] }]);
	});

	it('swallows a send that throws or rejects', async () => {
		const explode = () => {
			throw new Error('boom');
		};
		const throwing = new UsageTracker(explode, 100);
		throwing.track('export.open');
		const rejecting = new UsageTracker(() => Promise.reject(new Error('down')), 100);
		rejecting.track('export.open');
		expect(() => vi.advanceTimersByTime(100)).not.toThrow();
		await vi.advanceTimersByTimeAsync(0);
	});
});

describe('leaving the page', () => {
	/** A window and a document that only record their listeners, which the test fires. */
	function page(visibility: 'visible' | 'hidden') {
		const listeners: Record<string, () => void> = {};
		const target = (kind: string) => ({
			addEventListener: (type: string, listener: () => void) => {
				listeners[kind + type] = listener;
			}
		});
		return {
			win: target('win:'),
			doc: { ...target('doc:'), visibilityState: visibility },
			fire: (name: string) => listeners[name]()
		};
	}

	it('sends what is waiting on pagehide', () => {
		const sent: UsageIn[] = [];
		const tracker = new UsageTracker((report) => sent.push(report), 10_000);
		const { win, doc, fire } = page('visible');
		flushOnLeave(tracker, win, doc);
		tracker.track('export.open');
		fire('win:pagehide');
		expect(sent).toHaveLength(1);
	});

	it('sends it when the page becomes hidden, not when it becomes visible', () => {
		const sent: UsageIn[] = [];
		const tracker = new UsageTracker((report) => sent.push(report), 10_000);
		const visible = page('visible');
		flushOnLeave(tracker, visible.win, visible.doc);
		tracker.track('export.open');
		visible.fire('doc:visibilitychange');
		expect(sent).toHaveLength(0);

		const hidden = page('hidden');
		flushOnLeave(tracker, hidden.win, hidden.doc);
		hidden.fire('doc:visibilitychange');
		expect(sent).toHaveLength(1);
	});
});

describe('the request', () => {
	it('only ever goes to the relative /api/usage, with the header a write needs', async () => {
		const { fetch, calls } = fakeFetch();
		vi.stubGlobal('fetch', fetch);
		track('copy.prompt');
		track('panel.seen.rhythm');
		track('reader.tab.events');
		await vi.advanceTimersByTimeAsync(10_000);
		await postUsage({ hits: [{ action: 'export.open', n: 1 }] });
		expect(calls).toHaveLength(2);
		for (const call of calls) {
			expect(call.url).toBe('/api/usage');
			expect(call.method).toBe('POST');
			expect(call.headers['x-gitprompt']).toBe('1');
		}
		expect(calls[0].body).toEqual({
			hits: [
				{ action: 'copy.prompt', n: 1 },
				{ action: 'panel.seen.rhythm', n: 1 },
				{ action: 'reader.tab.events', n: 1 }
			]
		});
	});

	it('is a path on the daemon that served the page, never an address', () => {
		expect(USAGE_URL).toBe('/api/usage');
		expect(USAGE_URL).not.toMatch(/^(\/\/|[a-z]+:)/i);
	});
});
