import { STREAM_URL } from '#lib/api/client.ts';

/** How long to wait before reopening a stream the server closed for good (an HTTP error). */
export const RETRY_MS = 5_000;

/**
 * The daemon's "something was captured" stream, shared by the whole dashboard: one EventSource,
 * opened while anything listens. `state` and `lastNotice` are reactive (the sidebar shows them).
 */
class Live {
	state = $state<'connecting' | 'open' | 'reconnecting' | 'closed'>('closed');
	/** When the last notice arrived (ms), null before the first one. */
	lastNotice = $state<number | null>(null);

	#source: EventSource | null = null;
	#retry: ReturnType<typeof setTimeout> | undefined;
	#listeners = new Set<() => void>();

	#open() {
		if (this.#source || typeof EventSource === 'undefined') return;
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
				if (this.#listeners.size) this.#open();
			}, RETRY_MS);
		};
		source.onmessage = () => {
			this.state = 'open';
			this.lastNotice = Date.now();
			for (const listener of this.#listeners) listener();
		};
		this.#source = source;
	}

	#close() {
		clearTimeout(this.#retry);
		this.#source?.close();
		this.#source = null;
		this.state = 'closed';
	}

	/** Call `listener` on every notice; returns the unsubscribe function. */
	subscribe(listener: () => void): () => void {
		this.#listeners.add(listener);
		this.#open();
		return () => {
			this.#listeners.delete(listener);
			if (!this.#listeners.size) this.#close();
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
