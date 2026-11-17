/**
 * The desktop app's answers. The shell puts one global object in the page (`__TAURI__`, which
 * only this module may name); it is read as `unknown` and narrowed here, so nothing else in the
 * dashboard has to trust its shape. Calls name the app's own commands (backend/apps/desktop).
 */
import { browserPlatform, download, type BrowserPage } from '#lib/platform/browser.ts';
import type { Platform, Unsubscribe } from '#lib/platform/types.ts';

/** Starts listening to an app event; resolves to the function that stops it. */
type Listen = (
	event: string,
	handler: (event: { payload: unknown }) => void
) => Promise<() => void>;

/** The two pieces of the shell's global object the dashboard uses. */
export type Shell = {
	invoke: (command: string, args?: Record<string, unknown>) => Promise<unknown>;
	listen: Listen;
};

const isRecord = (value: unknown): value is Record<string, unknown> =>
	typeof value === 'object' && value !== null;

// What the shell documents for `core` and `event`; a guard checks the one function each is read for.
type Core = { invoke: Shell['invoke'] };
type Events = { listen: Listen };
const isCore = (value: unknown): value is Core =>
	isRecord(value) && typeof value.invoke === 'function';
const isEvents = (value: unknown): value is Events =>
	isRecord(value) && typeof value.listen === 'function';

/** The shell's `invoke` and `listen` in a global scope, or undefined when there is no shell. */
export function shellIn(scope: unknown): Shell | undefined {
	if (!isRecord(scope) || !isRecord(scope.__TAURI__)) return undefined;
	const { core, event } = scope.__TAURI__;
	if (!isCore(core) || !isEvents(event)) return undefined;
	return {
		invoke: (command, args) => core.invoke(command, args),
		listen: (name, handler) => event.listen(name, handler)
	};
}

/** Listen now; the returned function stops it, even when called before the listening began. */
function subscribe(
	shell: Shell,
	events: string[],
	handler: (event: string, payload: unknown) => void
) {
	let stopped = false;
	const stops: Array<() => void> = [];
	for (const name of events) {
		void shell
			.listen(name, (e) => handler(name, e.payload))
			.then((stop) => (stopped ? stop() : stops.push(stop)))
			.catch(() => {});
	}
	const unsubscribe: Unsubscribe = () => {
		stopped = true;
		for (const stop of stops.splice(0)) stop();
	};
	return unsubscribe;
}

/** Why a call failed: the app rejects with the sentence itself; anything else says what it is. */
const reasonOf = (value: unknown): string =>
	typeof value === 'string' ? value : value instanceof Error ? value.message : String(value);

/** The desktop answers: the shell for what only it can do, the browser's answer for the rest. */
export function desktopPlatform(shell: Shell, page: BrowserPage): Platform {
	return {
		...browserPlatform(page),
		desktop: true,
		hideWindow: () => shell.invoke('hide_window').then(() => {}),
		// No native save dialog yet: the download the web view offers, until a ticket needs one.
		saveFile: (file) => Promise.resolve(download(page, file)),
		onNavigate: (handler) =>
			subscribe(
				shell,
				['gitprompt://navigate'],
				(_, href) => typeof href === 'string' && handler(href)
			),
		// A window the user can see is a focused one: the app shows it to be used.
		onWindowVisible: (handler) =>
			subscribe(shell, ['tauri://focus', 'tauri://blur'], (name) =>
				handler(name === 'tauri://focus')
			),
		recall: {
			problem: () =>
				shell
					.invoke('recall_status')
					.then((reason) => (typeof reason === 'string' ? reason : null)),
			register: (shortcut) =>
				shell.invoke('recall_register', { shortcut }).then(
					() => ({ kind: 'registered' }),
					(reason: unknown) => ({ kind: 'refused', reason: reasonOf(reason) })
				)
		}
	};
}
