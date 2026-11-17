/**
 * The palette's two modes. `full` is ⌘K: everything, with commands. `recall` is the desktop
 * recall window (`/?recall=1`): prompts only, ↵ copies, no prefixes. The mode only changes what
 * is asked, listed and done; the dialog is the same one.
 */
import { actionsFor, type ActionKey, type Kind, type StoredAction } from '#lib/palette/actions.ts';
import type { FindKind } from '#lib/api/client.ts';
import { kindsFor, parseQuery, type Parsed } from '#lib/palette/query.ts';
import type { View } from '#lib/palette/rows.ts';
import type { Pending } from '#lib/shared/clipboard.ts';

export type PaletteMode = 'full' | 'recall';

/** The page's query parameter that asks for recall mode: `/?recall=1`. */
const PARAM = 'recall';

export const isRecall = (url: { searchParams: Pick<URLSearchParams, 'get'> }): boolean =>
	url.searchParams.get(PARAM) === '1';

/** What was typed: recall has no prefixes, so `>` is just a character to search for. */
export function parseFor(mode: PaletteMode, raw: string): Parsed {
	return mode === 'recall' ? { mode: 'all', text: raw.trim() } : parseQuery(raw);
}

/** What to ask the API for. */
export function kindsIn(mode: PaletteMode, parsed: Parsed): FindKind[] {
	return mode === 'recall' ? ['prompts'] : kindsFor(parsed.mode);
}

/** Recall lists prompts and nothing else: no sessions, and no "search the Prompts page" way out. */
export function restrict(mode: PaletteMode, view: View): View {
	if (mode === 'full') return view;
	return { ...view, groups: view.groups.filter((group) => group.kind === 'prompt') };
}

/** The key an action runs on: in recall ↵ copies, which is what ⌘↵ does everywhere else. */
export function actionKeyIn(mode: PaletteMode, key: ActionKey): ActionKey {
	return mode === 'recall' && key === 'enter' ? 'mod+enter' : key;
}

/** The actions listed for a row, with the key each is shown on (recall: copy is ↵). */
export function actionsIn(mode: PaletteMode, kind: Kind): StoredAction[] {
	const all = actionsFor(kind);
	if (mode === 'full') return all;
	return all.filter((a) => a.key === 'mod+enter').map((a) => ({ ...a, key: 'enter' }));
}

/**
 * Copying in recall mode: the window stays until the text is on the clipboard, then `done` runs
 * (it closes the palette, which hides the window). A copy that fails leaves the window open so
 * its message can be read.
 */
export function copyThenDone(
	copy: (text: Pending) => Promise<boolean>,
	done: () => void
): (text: Pending) => Promise<boolean> {
	return async (text) => {
		const copied = await copy(text);
		if (copied) done();
		return copied;
	};
}

/** What the dialog and its box say. */
export type Words = { dialog: string; label: string; placeholder: string; empty: string };

const WORDS: Record<PaletteMode, Words> = {
	full: {
		dialog: 'Find or run a command',
		label: 'Find a prompt, session or project. Start with > for commands.',
		placeholder: 'Find anything, or type > for commands',
		empty: 'Nothing captured yet.'
	},
	recall: {
		dialog: 'Find a prompt to copy',
		label: 'Find a prompt to copy. Enter copies it.',
		placeholder: 'Find a prompt to copy',
		empty: 'No prompt captured yet.'
	}
};

export const wordsFor = (mode: PaletteMode): Words => WORDS[mode];
