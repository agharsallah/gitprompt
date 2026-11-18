/**
 * Counting what gets used, on this machine only. `track(action)` adds one to today's count of an
 * action in the daemon's own database (`POST /api/usage`, a relative URL: the request never leaves
 * localhost) and is read back in Settings, Developer. It never throws and never waits: counts are
 * batched, sent a moment later and when the page is hidden, and a failure is dropped.
 */
import type { UsageHit, UsageIn } from '#lib/api/types.ts';
import type { UsageAction } from '#lib/shared/usage-actions.ts';

/** The daemon's endpoint. Relative on purpose: counts go to the daemon that served the page. */
export const USAGE_URL = '/api/usage';

/** How long counts wait for company before they are sent. */
export const FLUSH_MS = 2000;

/** The daemon takes at most 1000 of a name per report, so a count that reaches it is sent at once. */
const MAX_COUNT = 1000;

/** The daemon takes at most 64 names per report (service/usage.rs `MAX_HITS`): more go in another. */
export const MAX_HITS = 64;

export type Send = (report: UsageIn) => unknown;

/** The report, to the daemon: with the header every write needs, kept alive across a page hide. */
export function postUsage(report: UsageIn): Promise<unknown> {
	return fetch(USAGE_URL, {
		method: 'POST',
		headers: { 'X-GitPrompt': '1', 'Content-Type': 'application/json' },
		body: JSON.stringify(report),
		keepalive: true
	});
}

/** Counts waiting to be sent: grouped per action, flushed after `delayMs` and on `flush()`. */
export class UsageTracker {
	#pending = new Map<UsageAction, number>();
	#timer: ReturnType<typeof setTimeout> | undefined;
	#send: Send;
	#delayMs: number;

	constructor(send: Send = postUsage, delayMs: number = FLUSH_MS) {
		this.#send = send;
		this.#delayMs = delayMs;
	}

	track(action: UsageAction): void {
		const n = (this.#pending.get(action) ?? 0) + 1;
		this.#pending.set(action, n);
		if (n >= MAX_COUNT) this.flush();
		else this.#timer ??= setTimeout(() => this.flush(), this.#delayMs);
	}

	/** Send what is waiting, now. */
	flush(): void {
		clearTimeout(this.#timer);
		this.#timer = undefined;
		const hits: UsageHit[] = [...this.#pending].map(([action, n]) => ({ action, n }));
		this.#pending.clear();
		for (let at = 0; at < hits.length; at += MAX_HITS) {
			this.#deliver({ hits: hits.slice(at, at + MAX_HITS) });
		}
	}

	#deliver(report: UsageIn) {
		try {
			Promise.resolve(this.#send(report)).catch(() => {});
		} catch {
			// Counting is best effort: a page never fails because of it.
		}
	}
}

/** The part of a window or a document that `flushOnLeave` uses. */
type Listens = { addEventListener: (type: string, listener: () => void) => void };

/** Send what is waiting when the page is hidden or closed (the last chance to). */
export function flushOnLeave(
	tracker: UsageTracker,
	win: Listens,
	doc: Listens & { visibilityState: string }
): void {
	win.addEventListener('pagehide', () => tracker.flush());
	doc.addEventListener('visibilitychange', () => {
		if (doc.visibilityState === 'hidden') tracker.flush();
	});
}

const tracker = new UsageTracker();
if (typeof window !== 'undefined') flushOnLeave(tracker, window, document);

/** Count one use of `action`, today. */
export const track = (action: UsageAction): void => tracker.track(action);
