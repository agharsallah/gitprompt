import { STREAM_URL } from '#lib/api/client.ts';
import { parseNotification } from '#lib/api/notification.ts';
import type { Notification } from '#lib/api/types.ts';

/** How long to wait before reopening a stream the server closed for good (an HTTP error). */
export const RETRY_MS = 5_000;

/** What the stream is doing: `open` while notices can arrive. */
export type StreamState = 'connecting' | 'open' | 'reconnecting' | 'closed';

/**
 * The daemon's "something was captured" stream, shared by the whole dashboard: one EventSource,
 * opened while anything listens. `state` and `lastNotice` are reactive (the sidebar shows them).
 * The same stream carries the notifications the daemon decided on, as named events.
 */
class Live {
	state = $state<StreamState>('closed');
	/** When the last notice arrived (ms), null before the first one. */
	lastNotice = $state<number | null>(null);

	#source: EventSource | null = null;
	#retry: ReturnType<typeof setTimeout> | undefined;
	/** The window is hidden: no stream, whoever listens. */
	#hidden = false;
	#listeners = new Set<() => void>();
	#notified = new Set<(notification: Notification) => void>();

	get #listening() {
		return this.#listeners.size + this.#notified.size > 0;
	}

	#open() {
		if (this.#hidden || this.#source || typeof EventSource === 'undefined') return;
		this.state = 'connecting';
		const source = new EventSource(STREAM_URL);
		source.onopen = () => (this.state = 'open');
		// EventSource reconnects on its own after a network error; report it as reconnecting meanwhile.
		// After an HTTP error it gives up (CLOSED): drop it and open a new one later, or live
		// updates would stop until every listener left.
		source.onerror = () => {
			this.state = 'reconnecting';
			if (source.readyState !== EventSource.CLOSED) return;
			source.close();
			if (this.#source === source) this.#source = null;
			clearTimeout(this.#retry);
			this.#retry = setTimeout(() => {
				if (this.#listening) this.#open();
			}, RETRY_MS);
		};
		source.onmessage = () => {
			this.state = 'open';
			this.lastNotice = Date.now();
			for (const listener of this.#listeners) listener();
		};
		source.addEventListener('notification', (event) => {
			const notification =
				event instanceof MessageEvent ? parseNotification(String(event.data)) : null;
			if (!notification) return;
			for (const listener of this.#notified) listener(notification);
		});
		this.#source = source;
	}

	#close() {
		clearTimeout(this.#retry);
		this.#source?.close();
		this.#source = null;
		this.state = 'closed';
	}

	/**
	 * The window was hidden or shown (`platform.onWindowVisible`). Hidden, the stream closes and the
	 * page asks the daemon nothing; shown again, it reopens and every listener runs once, since
	 * notices sent meanwhile were missed.
	 */
	setVisible(visible: boolean) {
		if (visible === !this.#hidden) return;
		this.#hidden = !visible;
		if (!visible) return this.#close();
		if (!this.#listening) return;
		this.#open();
		for (const listener of this.#listeners) listener();
	}

	/** Call `listener` on every notice; returns the unsubscribe function. */
	subscribe(listener: () => void): () => void {
		this.#listeners.add(listener);
		this.#open();
		return () => {
			this.#listeners.delete(listener);
			if (!this.#listening) this.#close();
		};
	}

	/** Call `listener` on every notification; returns the unsubscribe function. */
	onNotification(listener: (notification: Notification) => void): () => void {
		this.#notified.add(listener);
		this.#open();
		return () => {
			this.#notified.delete(listener);
			if (!this.#listening) this.#close();
		};
	}
}

export const live = new Live();

/**
 * Call `onchange` when the daemon reports new prompts or events, once a burst of notices settles
 * (a turn produces many events). Returns the cleanup function, so it fits `$effect` directly:
 *
 *     $effect(() => liveUpdates(reload));
 */
export function liveUpdates(onchange: () => unknown, settleMs = 400): () => void {
	let timer: ReturnType<typeof setTimeout>;
	const unsubscribe = live.subscribe(() => {
		clearTimeout(timer);
		timer = setTimeout(() => void onchange(), settleMs);
	});
	return () => {
		clearTimeout(timer);
		unsubscribe();
	};
}
