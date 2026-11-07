import { api } from '#lib/api/client.ts';
import type { Prompt } from '#lib/api/types.ts';
import { rangeStart, type Range } from '#lib/insights/range.ts';

type Fetch = typeof fetch;

/** The API's page size limit (api/routes/prompts.py). */
const PAGE = 200;
/** Never load more than this many prompts for the insight pages. */
export const CAP = 1000;

export type Recent = {
	prompts: Prompt[];
	/** True when the window holds more than CAP prompts: the stories cover the newest CAP. */
	capped: boolean;
	start: Date | null;
	loadedAt: number;
};

/** Every prompt received in the window, newest first, paging with `before`. */
async function load(range: Range, fetcher: Fetch): Promise<Recent> {
	const now = Date.now();
	const start = rangeStart(range, now);
	const prompts: Prompt[] = [];
	let before: number | undefined;
	while (prompts.length < CAP) {
		const params = new URLSearchParams({ limit: String(PAGE) });
		if (before !== undefined) params.set('before', String(before));
		const page = await api.prompts(params, fetcher);
		const inside = start ? page.filter((p) => Date.parse(p.received_at) >= start.getTime()) : page;
		prompts.push(...inside);
		if (page.length < PAGE || inside.length < page.length) {
			return { prompts: prompts.slice(0, CAP), capped: false, start, loadedAt: now };
		}
		before = page.at(-1)!.id;
	}
	return { prompts: prompts.slice(0, CAP), capped: true, start, loadedAt: now };
}

// Overview, Sessions and Projects read the same window: share one load between them until something
// new is captured (live.svelte.ts calls invalidateRecent) or it is a minute old.
const TTL_MS = 60_000;
const cache = new Map<string, Promise<Recent>>();
const loadedAt = new Map<string, number>();

export function recent(range: Range, fetcher: Fetch = fetch): Promise<Recent> {
	const fresh = Date.now() - (loadedAt.get(range.id) ?? 0) < TTL_MS;
	if (!fresh || !cache.has(range.id)) {
		const pending = load(range, fetcher);
		cache.set(range.id, pending);
		loadedAt.set(range.id, Date.now());
		pending.catch(() => cache.delete(range.id));
	}
	return cache.get(range.id)!;
}

export function invalidateRecent() {
	cache.clear();
	loadedAt.clear();
}
