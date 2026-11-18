/**
 * Worth a look: the one button of each kind of item. The map from kind to action is here and only
 * here; a kind it does not know (a new source in the daemon, an older dashboard) is a plain link
 * to the item's target, so a new source never breaks the page.
 */
import type { WorthAction, WorthItem, WorthKind } from '#lib/api/types.ts';
import { copyCounted } from '#lib/shared/clipboard.ts';
import { promptHref, targetHref } from '#lib/shared/links.ts';
import type { UsageAction } from '#lib/shared/usage-actions.ts';
import { toasts } from '#lib/ui/toast.svelte.ts';

/** What a row's button does, ready to draw. */
export type WorthButton =
	/** Copy a prompt as a ready command file: GitPrompt never writes the file, `file` is a hint. */
	| { type: 'copy'; label: string; text: string; file: string }
	/** Open the turn in the reader. */
	| { type: 'open'; label: string; href: string }
	/** Hide the values of these findings, after you confirm (leaks/HideValues.svelte). */
	| { type: 'hide'; label: string; findings: string[] }
	/** A kind this dashboard does not know: a link to where the item leads. */
	| { type: 'link'; label: string; href: string };

/** The action each known kind takes: an item whose action is another one is shown as a link. */
const ACTION_OF: Record<WorthKind, WorthAction['type']> = {
	possible_leak: 'hide_values',
	redone_turn: 'open_turn',
	repeated_prompt: 'copy_command'
};

const isKnown = (kind: string): kind is WorthKind => Object.hasOwn(ACTION_OF, kind);

/** Where Claude Code reads a command file of this name (the suggestion in the copy's hint). */
export const commandFile = (name: string) => `.claude/commands/${name}.md`;

function button(action: WorthAction): WorthButton {
	switch (action.type) {
		case 'copy_command':
			return {
				type: 'copy',
				label: 'Copy as command',
				text: action.text,
				file: commandFile(action.name)
			};
		case 'open_turn':
			return { type: 'open', label: 'Open the turn', href: promptHref(action.prompt_id) };
		case 'hide_values':
			return { type: 'hide', label: 'Hide values', findings: action.findings };
	}
}

export function buttonFor(item: WorthItem): WorthButton {
	if (isKnown(item.kind) && ACTION_OF[item.kind] === item.action.type) return button(item.action);
	return { type: 'link', label: 'Have a look', href: targetHref(item.target) };
}

/** What acting on an item of this kind is counted as (usage-actions.ts). */
export const actionCount = (kind: WorthKind): UsageAction => `worth.act.${kind}`;

/**
 * Copy the command's text and say where to save it. Call it in the click handler itself (the
 * clipboard refuses a write after an `await`). False, and nothing counted, when it was refused.
 */
export async function copyCommand(
	kind: WorthKind,
	copy: Extract<WorthButton, { type: 'copy' }>
): Promise<boolean> {
	const done = await copyCounted(copy.text, actionCount(kind));
	if (done) toasts.show(`Copied · save it as ${copy.file} to use it as a command`);
	else toasts.show('The clipboard refused the copy', { tone: 'error' });
	return done;
}
