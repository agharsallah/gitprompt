/**
 * Data a page streams in: its `load` returns a promise (wrapped in `defer`) instead of awaiting
 * it, so the page renders at once, and the page reads the promise through a Resource.
 *
 * A Resource keeps the last value it received. When `load` runs again (a live update, a retry, a
 * filter change) the old value stays on screen until the new one arrives, so the page never falls
 * back to a skeleton once it has shown data. An answer that arrives after a newer request is
 * dropped. A failed refresh keeps the old value next to the error.
 *
 *     // +page.ts
 *     return { filters, overview: defer(loadOverview(filters, fetch)) };
 *     // +page.svelte
 *     const overview = new Resource(() => data.overview);
 *     <Deferred resource={overview} label="Loading the overview">…</Deferred>
 */
import { untrack } from 'svelte';
import { page } from '$app/state';

/** Return a streamed promise from `load`: a rejection is reported by the page, not as unhandled. */
export function defer<T>(promise: Promise<T>): Promise<T> {
	promise.catch(() => {});
	return promise;
}

type Outcome<T> = {
	key: unknown;
	/** The URL the request was made for. */
	at: string;
	/** A value is available (a fresh one, or the last good one when a refresh failed). */
	has: boolean;
	value?: T;
	failed: boolean;
	error?: unknown;
};

export class Resource<T> {
	#key: () => unknown;
	#settled = $state.raw<Outcome<T> | null>(null);
	#inflight = $state.raw<{ at: string } | null>(null);

	/**
	 * `source` returns the current promise (usually a field of `data`). `key` names what the data is
	 * about (a session id): when it changes, the old value is not shown for the new key.
	 */
	constructor(source: () => Promise<T>, key: () => unknown = () => undefined) {
		this.#key = key;
		$effect(() => {
			const promise = source();
			const k = key();
			const at = untrack(() => page.url.href);
			this.#inflight = { at };
			let current = true;
			promise.then(
				(value) => {
					if (!current) return;
					this.#settled = { key: k, at, has: true, value, failed: false };
					this.#inflight = null;
				},
				(error) => {
					if (!current) return;
					console.error('GitPrompt: loading failed', error);
					const last = this.#settled;
					const keep = !!last?.has && Object.is(last.key, k);
					this.#settled = {
						key: k,
						at,
						has: keep,
						value: keep ? last.value : undefined,
						failed: true,
						error
					};
					this.#inflight = null;
				}
			);
			return () => {
				current = false;
			};
		});
	}

	get #current(): Outcome<T> | null {
		const settled = this.#settled;
		return settled && Object.is(settled.key, this.#key()) ? settled : null;
	}

	/** A value can be shown (possibly an older one, see `loading` and `failed`). */
	get ready(): boolean {
		return !!this.#current?.has;
	}

	get value(): T | undefined {
		const current = this.#current;
		return current?.has ? current.value : undefined;
	}

	/** The last request failed. With `ready`, the value shown is from before the failure. */
	get failed(): boolean {
		return !!this.#current?.failed;
	}

	get error(): unknown {
		return this.#current?.error;
	}

	/** A request is on its way. */
	get loading(): boolean {
		return this.#inflight !== null;
	}

	/**
	 * A request is on its way for another URL than the value shown (the filters changed), as opposed
	 * to a background refresh of the same view: only then is the old value dimmed.
	 */
	get changing(): boolean {
		const settled = this.#settled;
		return !!this.#inflight && !!settled && this.#inflight.at !== settled.at;
	}
}
