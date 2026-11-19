import { describe, expect, it, vi } from 'vitest';
import type { BrowserPage } from '#lib/platform/browser.ts';
import { platformFor } from '#lib/platform/index.ts';
import { desktopPlatform, shellIn, type Shell } from '#lib/platform/tauri.ts';

const page = {} as BrowserPage;

/** A shell that records its calls; `listen` keeps each handler so a test can send an event. */
function fakeShell(answers: Record<string, () => Promise<unknown>> = {}) {
	const calls: string[] = [];
	const handlers = new Map<string, (event: { payload: unknown }) => void>();
	const stopped: string[] = [];
	const shell: Shell = {
		invoke: (command, args) => {
			calls.push(args ? `${command} ${JSON.stringify(args)}` : command);
			return (answers[command] ?? (() => Promise.resolve(null)))();
		},
		listen: (name, handler) => {
			handlers.set(name, handler);
			return Promise.resolve(() => void stopped.push(name));
		}
	};
	return {
		shell,
		calls,
		stopped,
		send: (name: string, payload?: unknown) => handlers.get(name)?.({ payload })
	};
}

/** Lets the promise chains of `listen` finish (microtasks only: nothing waits on a clock). */
async function settle() {
	for (let i = 0; i < 5; i++) await Promise.resolve();
}

describe('finding the shell', () => {
	it('is the global object the shell injects, and nothing without it', () => {
		const scope = { __TAURI__: { core: { invoke: vi.fn() }, event: { listen: vi.fn() } } };
		expect(shellIn(scope)).toBeDefined();
		expect(shellIn({})).toBeUndefined();
		expect(shellIn(undefined)).toBeUndefined();
		expect(shellIn({ __TAURI__: {} })).toBeUndefined();
		expect(
			shellIn({ __TAURI__: { core: { invoke: 1 }, event: { listen: vi.fn() } } })
		).toBeUndefined();
		expect(shellIn({ __TAURI__: { core: { invoke: vi.fn() }, event: {} } })).toBeUndefined();
	});

	it('passes calls through to the shell as they are', async () => {
		const invoke = vi.fn(() => Promise.resolve('ok'));
		const shell = shellIn({ __TAURI__: { core: { invoke }, event: { listen: vi.fn() } } });
		await expect(shell?.invoke('hide_window', { a: 1 })).resolves.toBe('ok');
		expect(invoke).toHaveBeenCalledWith('hide_window', { a: 1 });
	});

	it('chooses the desktop answers only when the shell is there', () => {
		const withShell = { __TAURI__: { core: { invoke: vi.fn() }, event: { listen: vi.fn() } } };
		expect(platformFor(withShell, page).desktop).toBe(true);
		expect(platformFor({}, page).desktop).toBe(false);
	});
});

describe('the desktop answers', () => {
	it('hides the window through the app', async () => {
		const { shell, calls } = fakeShell();
		await desktopPlatform(shell, page).hideWindow();
		expect(calls).toEqual(['hide_window']);
	});

	it('hands a navigation request to the page, only when it carries a URL, until stopped', async () => {
		const { shell, send, stopped } = fakeShell();
		const seen: string[] = [];
		const stop = desktopPlatform(shell, page).onNavigate((href) => seen.push(href));
		await settle();
		send('gitprompt://navigate', '/sessions/');
		send('gitprompt://navigate', 42);
		stop();
		await settle();
		expect(seen).toEqual(['/sessions/']);
		expect(stopped).toEqual(['gitprompt://navigate']);
	});

	it('hands the page what the app says when it shows or hides the window', async () => {
		const { shell, send } = fakeShell();
		const seen: boolean[] = [];
		desktopPlatform(shell, page).onWindowVisible((visible) => seen.push(visible));
		await settle();
		send('gitprompt://visible', true);
		send('gitprompt://visible', false);
		send('gitprompt://visible', 'yes');
		expect(seen).toEqual([true, false]);
	});

	it('stops listening even when stopped before the shell answered', async () => {
		const { shell, stopped } = fakeShell();
		desktopPlatform(shell, page).onWindowVisible(() => {})();
		await settle();
		expect(stopped).toEqual(['gitprompt://visible']);
	});

	it('asks the app for the recall shortcut and reports a refusal as its reason', async () => {
		const refusing = fakeShell({
			recall_register: () => Promise.reject(new Error('Ctrl+Alt+K is not available')),
			recall_status: () => Promise.resolve('taken at start')
		});
		const { recall } = desktopPlatform(refusing.shell, page);
		await expect(recall.register('Ctrl+Alt+K')).resolves.toEqual({
			kind: 'refused',
			reason: 'Ctrl+Alt+K is not available'
		});
		await expect(recall.problem()).resolves.toBe('taken at start');
		expect(refusing.calls).toEqual(['recall_register {"shortcut":"Ctrl+Alt+K"}', 'recall_status']);

		const fine = fakeShell({ recall_register: () => Promise.resolve(null) });
		await expect(desktopPlatform(fine.shell, page).recall.register('Ctrl+Alt+P')).resolves.toEqual({
			kind: 'registered'
		});
		await expect(desktopPlatform(fine.shell, page).recall.problem()).resolves.toBeNull();
	});
});
