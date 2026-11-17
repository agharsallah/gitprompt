/**
 * `platform`: the one way the dashboard reaches the app it runs in (types.ts). Which answers it
 * gives is decided the first time it is used: the desktop app's when the shell's global object
 * is in the page, the browser's otherwise.
 */
import { browserPlatform, type BrowserPage } from '#lib/platform/browser.ts';
import { desktopPlatform, shellIn } from '#lib/platform/tauri.ts';
import type { Platform } from '#lib/platform/types.ts';

export type { Platform, SavedFile, ShortcutResult, Unsubscribe } from '#lib/platform/types.ts';

/** The answers for a global scope (the window, or a test's stand-in) and the page they use. */
export function platformFor(scope: unknown, page: BrowserPage): Platform {
	const shell = shellIn(scope);
	return shell ? desktopPlatform(shell, page) : browserPlatform(page);
}

let chosen: Platform | undefined;
const current = (): Platform =>
	(chosen ??= platformFor(globalThis, { document: globalThis.document, url: URL }));

export const platform: Platform = {
	get desktop() {
		return current().desktop;
	},
	hideWindow: () => current().hideWindow(),
	saveFile: (file) => current().saveFile(file),
	onNavigate: (handler) => current().onNavigate(handler),
	onWindowVisible: (handler) => current().onWindowVisible(handler),
	recall: {
		problem: () => current().recall.problem(),
		register: (shortcut) => current().recall.register(shortcut)
	}
};
