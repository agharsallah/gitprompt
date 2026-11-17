import { describe, expect, it } from 'vitest';
import { shortcutFromPress, shortcutLabel, type Press } from '#lib/settings/shortcut.ts';

const press = (code: string, mods: Partial<Press> = {}): Press => ({
	key: '',
	code,
	metaKey: false,
	ctrlKey: false,
	altKey: false,
	shiftKey: false,
	...mods
});

describe('the shortcut a key press makes', () => {
	it('is ⌥⌘P on a Mac: ⌘ is the platform key', () => {
		expect(shortcutFromPress(press('KeyP', { metaKey: true, altKey: true }), true)).toBe(
			'CmdOrCtrl+Alt+P'
		);
	});

	it('is Ctrl Alt P elsewhere: Ctrl is the platform key', () => {
		expect(shortcutFromPress(press('KeyP', { ctrlKey: true, altKey: true }), false)).toBe(
			'CmdOrCtrl+Alt+P'
		);
	});

	it('keeps the real Ctrl on a Mac and the Windows key elsewhere', () => {
		expect(shortcutFromPress(press('KeyK', { ctrlKey: true, metaKey: true }), true)).toBe(
			'CmdOrCtrl+Ctrl+K'
		);
		expect(shortcutFromPress(press('KeyK', { metaKey: true, shiftKey: true }), false)).toBe(
			'Shift+Super+K'
		);
	});

	it('names digits, function keys and Space by what they are, not what they type', () => {
		expect(shortcutFromPress(press('Digit7', { altKey: true }), true)).toBe('Alt+7');
		expect(shortcutFromPress(press('F12', { shiftKey: true }), true)).toBe('Shift+F12');
		expect(shortcutFromPress(press('Space', { ctrlKey: true }), false)).toBe('CmdOrCtrl+Space');
	});

	it('waits while there is only a modifier, no modifier or a key it cannot name', () => {
		expect(shortcutFromPress(press('AltLeft', { altKey: true }), true)).toBeUndefined();
		expect(shortcutFromPress(press('KeyP'), true)).toBeUndefined();
		expect(shortcutFromPress(press('Enter', { altKey: true }), true)).toBeUndefined();
	});
});

describe('a shortcut written for people', () => {
	it('uses Apple glyphs in Apple order on a Mac', () => {
		expect(shortcutLabel('CmdOrCtrl+Alt+P', true)).toBe('⌥⌘P');
		expect(shortcutLabel('Alt+Shift+Ctrl+CmdOrCtrl+F5', true)).toBe('⌃⌥⇧⌘F5');
		expect(shortcutLabel('Super+Space', true)).toBe('⌘Space');
		expect(shortcutLabel('CmdOrCtrl+Super+K', true)).toBe('⌘K');
	});

	it('spells the keys out elsewhere', () => {
		expect(shortcutLabel('CmdOrCtrl+Alt+P', false)).toBe('Ctrl+Alt+P');
		expect(shortcutLabel('Shift+Super+K', false)).toBe('Shift+Win+K');
	});
});
