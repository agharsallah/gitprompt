/** Copying from the palette: onto the clipboard, then said in a toast with the size. */
import { writeClipboard, type Pending } from '#lib/shared/clipboard.ts';
import { plural } from '#lib/shared/format.ts';
import { track } from '#lib/shared/usage.ts';
import { toasts } from '#lib/ui/toast.svelte.ts';

/** "Copied · 1,240 characters". */
export const copiedMessage = (text: string): string =>
	`Copied · ${plural([...text].length, 'character')}`;

/** What copying needs from the page; the defaults are the real ones, tests pass their own. */
export type CopyDeps = {
	/** Starts the write at once; the text may still be on its way (shared/clipboard.ts). */
	write: (text: Pending) => Promise<void>;
	say: (message: string, tone: 'info' | 'error') => void;
	count: () => void;
};

const real: CopyDeps = {
	write: writeClipboard,
	say: (message, tone) => toasts.show(message, { tone }),
	count: () => track('palette.copy')
};

/**
 * Put `text` on the clipboard and say so; a refusal is said too, and not counted. The write starts
 * before anything is awaited, so a text that is still being fetched is copied within the key press
 * that asked for it. A text that never arrives is the caller's failure: its rejection is passed on.
 */
export async function copyText(text: Pending, deps: CopyDeps = real): Promise<boolean> {
	const writing = deps.write(text);
	// Without a text there is nothing the clipboard could refuse: only the fetch's failure is told.
	writing.catch(() => {});
	const value = await text;
	try {
		await writing;
	} catch {
		deps.say('Could not copy: the browser refused clipboard access', 'error');
		return false;
	}
	deps.count();
	deps.say(copiedMessage(value), 'info');
	return true;
}
