/**
 * The commands the palette can run after `>`. A feature registers its own, from a module named
 * `<what>.commands.ts` next to the code it drives (features.ts loads them all), or from a
 * component while it is on screen (the returned function takes the command back). There is no
 * central list: a new feature adds a file, never an edit here.
 *
 *     registerCommand({ id: 'theme.dark', title: 'Use the dark theme', keywords: ['color'], run });
 */
import { highlight, wordsOf } from '#lib/palette/highlight.ts';
import type { Segment } from '#lib/api/types.ts';

export type Command = {
	/** Unique and stable: registering the same id again replaces the command. */
	id: string;
	title: string;
	/** Other words that should find it (searched with the title). */
	keywords: readonly string[];
	run: () => unknown;
};

const registry = new Map<string, Command>();

/** Add a command; the function it returns removes it again (only if it is still this one). */
export function registerCommand(command: Command): () => void {
	registry.set(command.id, command);
	return () => {
		if (registry.get(command.id) === command) registry.delete(command.id);
	};
}

/** Every registered command, in the order they were registered. */
export const commands = (): Command[] => [...registry.values()];

export type CommandMatch = { command: Command; title: Segment[] };

/** How well the words match: 0 when one is missing, higher for a title that starts with them. */
function score(command: Command, words: readonly string[]): number {
	const title = command.title.toLowerCase();
	const haystack = `${title} ${command.keywords.join(' ').toLowerCase()}`;
	if (!words.every((w) => haystack.includes(w.toLowerCase()))) return 0;
	if (title.startsWith(words[0].toLowerCase())) return 3;
	return words.every((w) => title.includes(w.toLowerCase())) ? 2 : 1;
}

/** The commands the text finds, best first (ties keep registration order); no text: all of them. */
export function matchCommands(text: string, all: readonly Command[] = commands()): CommandMatch[] {
	const words = wordsOf(text);
	return all
		.map((command, order) => ({ command, order, rank: words.length ? score(command, words) : 1 }))
		.filter((m) => m.rank > 0)
		.sort((a, b) => b.rank - a.rank || a.order - b.order)
		.map(({ command }) => ({ command, title: highlight(command.title, words) }));
}
