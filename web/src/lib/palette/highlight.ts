/** Marking the words of a query inside a text, as the API marks its snippets (`Segment`). */
import type { Segment } from '#lib/api/types.ts';

const escape = (word: string) => word.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

/** The words of a query: split on white space, empty ones dropped. */
export const wordsOf = (text: string): string[] => text.split(/\s+/).filter(Boolean);

/** `text` cut into pieces, the ones that are one of `words` (any case) marked `hit`. */
export function highlight(text: string, words: readonly string[]): Segment[] {
	if (!words.length) return [{ text, hit: false }];
	const pattern = new RegExp(`(${words.map(escape).join('|')})`, 'gi');
	// Splitting on a capture group alternates between the text between matches and the matches.
	return text
		.split(pattern)
		.map((part, i) => ({ text: part, hit: i % 2 === 1 }))
		.filter((segment) => segment.text !== '');
}
