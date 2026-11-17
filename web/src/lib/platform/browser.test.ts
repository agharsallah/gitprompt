import { describe, expect, it, vi } from 'vitest';
import { browserPlatform, type BrowserPage } from '#lib/platform/browser.ts';

type Listener = () => void;

/** A page that records what the answers do to it. */
function fakePage(visibility: DocumentVisibilityState = 'visible') {
	const listeners = new Set<Listener>();
	const log: string[] = [];
	const link = {
		href: '',
		download: '',
		click: () => log.push(`click ${link.download} ${link.href}`),
		remove: () => log.push('remove')
	};
	const page = {
		document: {
			visibilityState: visibility,
			createElement: () => link,
			body: { append: () => log.push('append') },
			addEventListener: (_: string, listener: Listener) => listeners.add(listener),
			removeEventListener: (_: string, listener: Listener) => listeners.delete(listener)
		},
		url: {
			createObjectURL: () => 'blob:one',
			revokeObjectURL: (href: string) => log.push(`revoke ${href}`)
		}
	};
	return {
		page: page as unknown as BrowserPage,
		set: (state: DocumentVisibilityState) => (page.document.visibilityState = state),
		fire: () => listeners.forEach((listener) => listener()),
		listeners,
		log
	};
}

describe('the browser answers', () => {
	it('is not the desktop app', () => {
		expect(browserPlatform(fakePage().page).desktop).toBe(false);
	});

	it('hides nothing: a tab is not ours to hide', async () => {
		await expect(browserPlatform(fakePage().page).hideWindow()).resolves.toBeUndefined();
	});

	it('saves a file as a download through a link that is removed again', async () => {
		const { page, log } = fakePage();
		await browserPlatform(page).saveFile({
			name: 'prompts.md',
			type: 'text/markdown',
			data: '# hi'
		});
		expect(log).toEqual(['append', 'click prompts.md blob:one', 'remove', 'revoke blob:one']);
	});

	it('never navigates: the handler is not kept and stopping it is safe', () => {
		const handler = vi.fn();
		const stop = browserPlatform(fakePage().page).onNavigate(handler);
		expect(() => stop()).not.toThrow();
		expect(handler).not.toHaveBeenCalled();
	});

	it('reports visibility changes of the page, until stopped', () => {
		const fake = fakePage();
		const seen: boolean[] = [];
		const stop = browserPlatform(fake.page).onWindowVisible((visible) => seen.push(visible));
		fake.set('hidden');
		fake.fire();
		fake.set('visible');
		fake.fire();
		stop();
		fake.fire();
		expect(seen).toEqual([false, true]);
		expect(fake.listeners.size).toBe(0);
	});

	it('has no recall shortcut: nothing to report, and a request is unsupported', async () => {
		const { recall } = browserPlatform(fakePage().page);
		await expect(recall.problem()).resolves.toBeNull();
		await expect(recall.register('Ctrl+Alt+P')).resolves.toEqual({ kind: 'unsupported' });
	});
});
