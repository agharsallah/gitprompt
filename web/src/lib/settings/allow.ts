/**
 * The `leak_allow` preference as one text field: values separated by commas or new lines, each
 * trimmed, empty ones dropped, each kept once. Pure, tested in allow.test.ts.
 */

/** The values of the text, in the order typed. */
export function parseAllow(text: string): string[] {
	const values = text
		.split(/[,\n]/)
		.map((v) => v.trim())
		.filter(Boolean);
	return [...new Set(values)];
}

/** The list as the field shows it. */
export const allowText = (values: readonly string[]) => values.join(', ');

/** The text says the same list as the stored one: nothing to save. */
export const sameAllow = (text: string, stored: readonly string[]) =>
	allowText(parseAllow(text)) === allowText(stored);

/** The stored list in words, for a locked setting: "a@b.example, c@d.example", or "None". */
export const allowSummary = (values: readonly string[]) =>
	values.length ? values.join(', ') : 'None';
