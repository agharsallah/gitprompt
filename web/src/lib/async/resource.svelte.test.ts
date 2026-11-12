import { flushSync } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { defer, Resource } from '#lib/async/resource.svelte.ts';

const page = vi.hoisted(() => ({ url: new URL('http://x/overview') }));
vi.mock('$app/state', () => ({ page }));

/** What a page's `load` hands over: the current promise and what it is about. */
type Data = { promise: Promise<string>; key: string };

let stops: (() => void)[] = [];

/** A Resource reading `data`, as a page builds it from its `load` result. */
function track(first: Promise<string>, key = 'a') {
	const data: Data = $state({ promise: first, key });
	let resource!: Resource<string>;
	stops.push(
		$effect.root(() => {
			resource = new Resource(
				() => data.promise,
				() => data.key
			);
		})
	);
	flushSync();
	/** `load` ran again: a new promise, possibly about something else. */
	const reload = (promise: Promise<string>, nextKey = data.key) => {
		data.promise = promise;
		data.key = nextKey;
		flushSync();
	};
	return { resource, reload };
}

/** Let settled promises run their callbacks. */
const settle = () => new Promise<void>((resolve) => setTimeout(resolve));

beforeEach(() => {
	page.url = new URL('http://x/overview');
	vi.spyOn(console, 'error').mockImplementation(() => {});
});
afterEach(() => {
	stops.forEach((stop) => stop());
	stops = [];
	vi.restoreAllMocks();
});

describe('Resource', () => {
	it('is loading with nothing to show until the first answer', async () => {
		const first = Promise.withResolvers<string>();
		const { resource } = track(first.promise);
		expect(resource).toMatchObject({ ready: false, loading: true, failed: false });
		expect(resource.value).toBeUndefined();

		first.resolve('one');
		await settle();
		expect(resource).toMatchObject({ ready: true, value: 'one', loading: false, failed: false });
	});

	it('keeps the old value on screen while a refresh is on its way', async () => {
		const { resource, reload } = track(Promise.resolve('one'));
		await settle();
		const next = Promise.withResolvers<string>();
		reload(next.promise);
		expect(resource).toMatchObject({ ready: true, value: 'one', loading: true });

		next.resolve('two');
		await settle();
		expect(resource).toMatchObject({ value: 'two', loading: false });
	});

	it('drops an answer that arrives after a newer request', async () => {
		const slow = Promise.withResolvers<string>();
		const fast = Promise.withResolvers<string>();
		const { resource, reload } = track(slow.promise);
		reload(fast.promise);
		fast.resolve('new');
		await settle();
		slow.resolve('old');
		await settle();
		expect(resource.value).toBe('new');
	});

	it('drops a failure that arrives after a newer request', async () => {
		const slow = Promise.withResolvers<string>();
		const { resource, reload } = track(slow.promise);
		reload(Promise.resolve('new'));
		await settle();
		slow.reject(new Error('late'));
		await settle();
		expect(resource).toMatchObject({ value: 'new', failed: false });
		expect(console.error).not.toHaveBeenCalled();
	});

	it('reports a first failure with nothing to show', async () => {
		const error = new Error('daemon down');
		const { resource } = track(Promise.reject(error));
		await settle();
		expect(resource).toMatchObject({ ready: false, failed: true, error, loading: false });
		expect(resource.value).toBeUndefined();
		expect(console.error).toHaveBeenCalledWith('GitPrompt: loading failed', error);
	});

	it('keeps the last good value next to the error of a failed refresh', async () => {
		const { resource, reload } = track(Promise.resolve('one'));
		await settle();
		const error = new Error('refresh failed');
		reload(Promise.reject(error));
		await settle();
		expect(resource).toMatchObject({ ready: true, value: 'one', failed: true, error });

		reload(Promise.resolve('two'));
		await settle();
		expect(resource).toMatchObject({ value: 'two', failed: false, error: undefined });
	});

	it('never shows the value of another key', async () => {
		const { resource, reload } = track(Promise.resolve('session a'), 'a');
		await settle();
		const next = Promise.withResolvers<string>();
		reload(next.promise, 'b');
		expect(resource).toMatchObject({ ready: false, loading: true, failed: false });
		expect(resource.value).toBeUndefined();

		next.reject(new Error('gone'));
		await settle();
		expect(resource).toMatchObject({ ready: false, failed: true });
		expect(resource.value).toBeUndefined();
	});

	it('is changing only while a request for another URL is on its way', async () => {
		const { resource, reload } = track(Promise.resolve('all'));
		expect(resource.changing).toBe(false); // nothing shown yet: a skeleton, not a dimmed value
		await settle();

		const refresh = Promise.withResolvers<string>();
		reload(refresh.promise);
		expect(resource.changing).toBe(false); // a background refresh of the same view
		refresh.resolve('all again');
		await settle();

		page.url = new URL('http://x/overview?repo=r');
		const filtered = Promise.withResolvers<string>();
		reload(filtered.promise);
		expect(resource.changing).toBe(true);
		filtered.resolve('filtered');
		await settle();
		expect(resource.changing).toBe(false);
	});
});

describe('defer', () => {
	it('hands back the same promise, its rejection handled', async () => {
		const unhandled = vi.fn();
		process.on('unhandledRejection', unhandled);
		const promise = Promise.reject(new Error('later'));
		expect(defer(promise)).toBe(promise);
		await settle();
		await settle();
		process.off('unhandledRejection', unhandled);
		expect(unhandled).not.toHaveBeenCalled();
		await expect(promise).rejects.toThrow('later');
	});
});
