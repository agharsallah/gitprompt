/**
 * What the dashboard needs from the app it runs in, as one interface. In a browser each call has
 * a plain answer; in the desktop app it reaches the native shell. Components and modules call
 * `platform` (index.ts) and never read the shell's global object themselves: a guard test
 * (platform.guard.test.ts) fails on any other file that does.
 */

/** Stops what `onNavigate` or `onWindowVisible` started. */
export type Unsubscribe = () => void;

/** A file the user is to keep: its name, its media type and its content. */
export type SavedFile = { name: string; type: string; data: Blob | string };

/** What setting the recall shortcut came to. `unsupported`: this is not the desktop app. */
export type ShortcutResult =
	{ kind: 'unsupported' } | { kind: 'registered' } | { kind: 'refused'; reason: string };

export type Platform = {
	/** True in the desktop app: what only it can do (the recall shortcut) is worth showing. */
	readonly desktop: boolean;
	/** Hide the window the page is in. Browser: nothing (a tab is not ours to hide). */
	hideWindow(): Promise<void>;
	/** Let the user keep a file. Browser: a download. */
	saveFile(file: SavedFile): Promise<void>;
	/** The app asks the page to go to a dashboard URL (a tray item, a notification). Browser: never. */
	onNavigate(handler: (href: string) => void): Unsubscribe;
	/** The window was shown or hidden (desktop: by the app). Browser: `visibilitychange`. */
	onWindowVisible(handler: (visible: boolean) => void): Unsubscribe;
	/** The desktop app's recall shortcut. */
	recall: {
		/** Why the shortcut set at start is not held (another app has it), or null. */
		problem(): Promise<string | null>;
		/** Take this shortcut (`CmdOrCtrl+Alt+P`); the old one keeps working when it is refused. */
		register(shortcut: string): Promise<ShortcutResult>;
	};
};
