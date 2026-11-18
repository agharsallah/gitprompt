/**
 * What each kind of result can do, as data. A kind has a list of actions, each on a key; the dialog
 * runs whichever the key asks for and lists them in its footer, so it knows no action by name. A
 * feature adds one from its own module, with nothing edited here or in the dialog:
 *
 *     // sessions/resume.commands.ts (ticket 08)
 *     registerAction('session', {
 *         key: 'alt+enter',
 *         label: 'copy resume',
 *         run: (item, ctx) => ctx.copy(resumeCommand(item.id))
 *     });
 *
 * Registering a key a kind already has replaces that action. `ctx` is what the dialog lends an
 * action: it closes the dialog first, so an action never has to.
 */
import type { Command } from '#lib/palette/commands.ts';
import type { Pending } from '#lib/shared/clipboard.ts';
import { commitAnchor } from '#lib/projects/commits.ts';
import { projectHref, promptHref, sessionHref } from '#lib/shared/links.ts';

/** The result kinds, with what each carries to its actions. `page` is the "no match" way out. */
type Items = {
	prompt: { id: number };
	session: { id: string };
	project: { id: string };
	commit: { project: string; hash: string };
	command: { command: Command };
	page: { href: string };
};

export type Kind = keyof Items;
export type Item = { [K in Kind]: { kind: K } & Items[K] }[Kind];
export type ItemOf<K extends Kind> = Extract<Item, { kind: K }>;

export type ActionKey = 'enter' | 'mod+enter' | 'alt+enter';

/** The order actions are listed in (the footer). */
const KEY_ORDER: readonly ActionKey[] = ['enter', 'mod+enter', 'alt+enter'];

export type ActionContext = {
	/** Close the dialog and go to a URL built by shared/links.ts. */
	navigate: (href: string) => unknown;
	/**
	 * Close the dialog, put text on the clipboard and say so ("Copied · 1,240 characters"); true once
	 * it is there. Give it a text that is still being fetched as its promise, never after an
	 * `await`: the clipboard only takes a write that starts in the key press (shared/clipboard.ts).
	 */
	copy: (text: Pending) => Promise<boolean>;
	/** Close the dialog (for an action that does something else). */
	close: () => void;
	/** The full text of a prompt, as copied (without injected context). */
	promptText: (id: number) => Promise<string>;
	/** An action failed (its promise was rejected): say so. */
	fail: (error: unknown) => void;
};

export type KindAction<K extends Kind> = {
	key: ActionKey;
	/** Lowercase, short: shown next to the key in the footer ("open", "copy"). */
	label: string;
	run: (item: ItemOf<K>, ctx: ActionContext) => unknown;
};

/** An action as stored: it accepts any item and ignores the ones of another kind. */
export type StoredAction = {
	key: ActionKey;
	label: string;
	run: (item: Item, ctx: ActionContext) => unknown;
};

const isKind = <K extends Kind>(item: Item, kind: K): item is ItemOf<K> => item.kind === kind;

const table = new Map<Kind, StoredAction[]>();

/** Add an action to a kind; the function it returns takes it away again. */
export function registerAction<K extends Kind>(kind: K, action: KindAction<K>): () => void {
	const stored: StoredAction = {
		key: action.key,
		label: action.label,
		run: (item, ctx) => (isKind(item, kind) ? action.run(item, ctx) : undefined)
	};
	const rest = (table.get(kind) ?? []).filter((a) => a.key !== action.key);
	table.set(kind, [...rest, stored]);
	return () => {
		const now = table.get(kind) ?? [];
		if (now.includes(stored))
			table.set(
				kind,
				now.filter((a) => a !== stored)
			);
	};
}

const rank = (a: StoredAction) => KEY_ORDER.indexOf(a.key);

/** The actions of a kind, in key order (↵ first). */
export const actionsFor = (kind: Kind): StoredAction[] =>
	[...(table.get(kind) ?? [])].sort((a, b) => rank(a) - rank(b));

/**
 * Run what `key` asks for on `item`: `false` when its kind has no action on that key, else `true`
 * once the action is done (a rejection goes to `ctx.fail`). The action starts at once.
 */
export async function perform(item: Item, key: ActionKey, ctx: ActionContext): Promise<boolean> {
	const action = actionsFor(item.kind).find((a) => a.key === key);
	if (!action) return false;
	try {
		await action.run(item, ctx);
	} catch (error) {
		ctx.fail(error);
	}
	return true;
}

// The built-in actions: open what you found, copy a prompt, run a command.
registerAction('prompt', {
	key: 'enter',
	label: 'open',
	run: (i, ctx) => ctx.navigate(promptHref(i.id))
});
registerAction('prompt', {
	key: 'mod+enter',
	label: 'copy',
	run: (i, ctx) => ctx.copy(ctx.promptText(i.id))
});
registerAction('session', {
	key: 'enter',
	label: 'open',
	run: (i, ctx) => ctx.navigate(sessionHref(i.id))
});
registerAction('project', {
	key: 'enter',
	label: 'open',
	run: (i, ctx) => ctx.navigate(projectHref(i.id))
});
registerAction('commit', {
	key: 'enter',
	label: 'open',
	run: (i, ctx) =>
		ctx.navigate(projectHref(i.project, { tab: 'commits', anchor: commitAnchor(i.hash) }))
});
registerAction('page', { key: 'enter', label: 'open', run: (i, ctx) => ctx.navigate(i.href) });
registerAction('command', {
	key: 'enter',
	label: 'run',
	run: (i, ctx) => {
		ctx.close();
		return i.command.run();
	}
});
