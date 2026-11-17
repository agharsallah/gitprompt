/**
 * Writing text to the clipboard when the text may still be on its way (a fetch). Safari, and the
 * desktop app's web view on macOS, only allow a clipboard write that starts inside the key press or
 * click: one made after an `await` is refused. So the write starts at once and is handed the
 * promise of the text, which every engine accepts.
 */

/** Text to copy: here already, or promised. */
export type Pending = string | Promise<string>;

/** Put `text` on the clipboard. Call it in the event handler itself, before any `await`. */
export function writeClipboard(text: Pending): Promise<void> {
	if (typeof text === 'string') return navigator.clipboard.writeText(text);
	if (typeof ClipboardItem === 'undefined') {
		return text.then((value) => navigator.clipboard.writeText(value));
	}
	const blob = text.then((value) => new Blob([value], { type: 'text/plain' }));
	return navigator.clipboard.write([new ClipboardItem({ 'text/plain': blob })]);
}
