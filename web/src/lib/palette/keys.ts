/** Keyboard in the palette as plain functions: what a key press means, and where it moves. */
import type { ActionKey } from '#lib/palette/actions.ts';

export type KeyLike = {
	key: string;
	metaKey: boolean;
	ctrlKey: boolean;
	altKey: boolean;
	shiftKey: boolean;
	isComposing?: boolean;
};

/** ⌘K, or Ctrl K: opens (and closes) the palette from anywhere. */
export const isShortcut = (e: KeyLike): boolean =>
	(e.metaKey || e.ctrlKey) && !e.altKey && !e.shiftKey && e.key.toLowerCase() === 'k';

/** Which action a press of Enter asks for: Enter, ⌘/Ctrl Enter, or ⌥ Enter. */
export function actionKeyOf(e: KeyLike): ActionKey | undefined {
	if (e.key !== 'Enter' || e.shiftKey) return undefined;
	if (e.metaKey || e.ctrlKey) return e.altKey ? undefined : 'mod+enter';
	return e.altKey ? 'alt+enter' : 'enter';
}

export type Intent = { type: 'move'; delta: 1 | -1 } | { type: 'act'; key: ActionKey };

/** What a key press asks for; `undefined` for keys the input keeps (typing, caret, Esc: the dialog). */
export function intentOf(e: KeyLike): Intent | undefined {
	if (e.isComposing) return undefined;
	if (e.key === 'ArrowDown') return { type: 'move', delta: 1 };
	if (e.key === 'ArrowUp') return { type: 'move', delta: -1 };
	const key = actionKeyOf(e);
	return key && { type: 'act', key };
}

/** The highlighted row after moving by `delta`, wrapping at both ends (-1: none to move over). */
export function moveActive(active: number, delta: number, count: number): number {
	if (count <= 0) return -1;
	return (((active + delta) % count) + count) % count;
}

/** How a key is written on a keycap: ↵, ⌘↵ (Ctrl ↵ off a Mac), ⌥↵ (Alt ↵). */
export function keyLabel(key: ActionKey, mac: boolean): string {
	if (key === 'enter') return '↵';
	if (key === 'mod+enter') return mac ? '⌘↵' : 'Ctrl ↵';
	return mac ? '⌥↵' : 'Alt ↵';
}
