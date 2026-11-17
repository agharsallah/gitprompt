import { describe, expect, it } from 'vitest';
import {
	actionKeyOf,
	intentOf,
	highlightedIndex,
	isShortcut,
	keyLabel,
	moveActive,
	type KeyLike
} from '#lib/palette/keys.ts';

const press = (key: string, mods: Partial<KeyLike> = {}): KeyLike => ({
	key,
	metaKey: false,
	ctrlKey: false,
	altKey: false,
	shiftKey: false,
	...mods
});

describe('keys', () => {
	it('opens on ⌘K and Ctrl K, and on nothing else with K', () => {
		expect(isShortcut(press('k', { metaKey: true }))).toBe(true);
		expect(isShortcut(press('K', { ctrlKey: true }))).toBe(true);
		expect(isShortcut(press('k'))).toBe(false);
		expect(isShortcut(press('k', { metaKey: true, shiftKey: true }))).toBe(false);
		expect(isShortcut(press('j', { metaKey: true }))).toBe(false);
	});

	it('tells the action a press of Enter asks for', () => {
		expect(actionKeyOf(press('Enter'))).toBe('enter');
		expect(actionKeyOf(press('Enter', { metaKey: true }))).toBe('mod+enter');
		expect(actionKeyOf(press('Enter', { ctrlKey: true }))).toBe('mod+enter');
		expect(actionKeyOf(press('Enter', { altKey: true }))).toBe('alt+enter');
		expect(actionKeyOf(press('Enter', { shiftKey: true }))).toBeUndefined();
		expect(actionKeyOf(press('Enter', { metaKey: true, altKey: true }))).toBeUndefined();
		expect(actionKeyOf(press('a'))).toBeUndefined();
	});

	it('turns arrows and Enter into intents, and leaves typing and Esc alone', () => {
		expect(intentOf(press('ArrowDown'))).toEqual({ type: 'move', delta: 1 });
		expect(intentOf(press('ArrowUp'))).toEqual({ type: 'move', delta: -1 });
		expect(intentOf(press('Enter'))).toEqual({ type: 'act', key: 'enter' });
		expect(intentOf(press('x'))).toBeUndefined();
		expect(intentOf(press('Escape'))).toBeUndefined();
	});

	it('leaves Enter to an input method that is composing', () => {
		expect(intentOf({ ...press('Enter'), isComposing: true })).toBeUndefined();
	});

	it('moves over the rows and wraps at both ends', () => {
		expect(moveActive(0, 1, 3)).toBe(1);
		expect(moveActive(2, 1, 3)).toBe(0);
		expect(moveActive(0, -1, 3)).toBe(2);
		expect(moveActive(0, 1, 0)).toBe(-1);
	});

	it('keeps the highlight on the same row when the list changes under it (↓ while results load)', () => {
		expect(highlightedIndex(['a', 'b', 'c'], 'b')).toBe(1);
		// The new results moved it: it follows the row, so ↵ opens what the eye was on.
		expect(highlightedIndex(['x', 'a', 'b'], 'b')).toBe(2);
		// Its row is gone: the first row, never a different row at the same position.
		expect(highlightedIndex(['x', 'y', 'z'], 'b')).toBe(0);
		expect(highlightedIndex(['x', 'y'], undefined)).toBe(0);
		expect(highlightedIndex([], 'b')).toBe(-1);
	});

	it('writes keys on a keycap for the platform', () => {
		expect(keyLabel('enter', true)).toBe('↵');
		expect(keyLabel('mod+enter', true)).toBe('⌘↵');
		expect(keyLabel('mod+enter', false)).toBe('Ctrl ↵');
		expect(keyLabel('alt+enter', true)).toBe('⌥↵');
		expect(keyLabel('alt+enter', false)).toBe('Alt ↵');
	});
});
