/**
 * The recall shortcut as the Settings page records and shows it. The text is the one the backend
 * stores (`CmdOrCtrl+Alt+P`, gitprompt-core `shortcut`); the backend decides what is valid, this
 * only builds the text from a key press and writes it for a person.
 */
import type { KeyLike } from '#lib/palette/keys.ts';

export type Press = KeyLike & { code: string };

/** The key of a press as the backend spells it: a letter, a digit, F1 to F24 or Space. */
function keyOf(code: string): string | undefined {
	const named = /^(?:Key([A-Z])|Digit(\d)|(F\d{1,2}))$/.exec(code);
	if (named) return named[1] ?? named[2] ?? named[3];
	return code === 'Space' ? 'Space' : undefined;
}

/** `CmdOrCtrl` is the key a Mac calls ⌘ and elsewhere Ctrl, so one stored shortcut suits both. */
function modifiersOf(press: Press, mac: boolean): string[] {
	const modifiers: string[] = [];
	if (mac ? press.metaKey : press.ctrlKey) modifiers.push('CmdOrCtrl');
	if (mac && press.ctrlKey) modifiers.push('Ctrl');
	if (press.altKey) modifiers.push('Alt');
	if (press.shiftKey) modifiers.push('Shift');
	if (!mac && press.metaKey) modifiers.push('Super');
	return modifiers;
}

/** The shortcut a key press makes, or undefined while it is only modifiers or has none. */
export function shortcutFromPress(press: Press, mac: boolean): string | undefined {
	const key = keyOf(press.code);
	const modifiers = modifiersOf(press, mac);
	return key && modifiers.length ? [...modifiers, key].join('+') : undefined;
}

/** Apple's order and glyphs: ⌃⌥⇧⌘, then the key. */
const MAC_GLYPHS: [string, string][] = [
	['Ctrl', '⌃'],
	['Alt', '⌥'],
	['Shift', '⇧'],
	['CmdOrCtrl', '⌘'],
	['Super', '⌘']
];
const OTHER_NAMES: Record<string, string> = { CmdOrCtrl: 'Ctrl', Super: 'Win' };

/** How a stored shortcut is written for people: ⌥⌘P on a Mac, Ctrl+Alt+P elsewhere. */
export function shortcutLabel(text: string, mac: boolean): string {
	const parts = text.split('+');
	const key = parts.pop() ?? '';
	if (!mac) return [...parts.map((m) => OTHER_NAMES[m] ?? m), key].join('+');
	const glyphs = MAC_GLYPHS.filter(([name]) => parts.includes(name)).map(([, glyph]) => glyph);
	return [...new Set(glyphs)].join('') + key;
}
