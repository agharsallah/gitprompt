/** What the palette lists: the API's answer (or the commands) as groups of one-line rows. */
import type { FindHit, FindResults, Segment } from '#lib/api/types.ts';
import type { Item, Kind } from '#lib/palette/actions.ts';
import { matchCommands, type Command } from '#lib/palette/commands.ts';
import { highlight, wordsOf } from '#lib/palette/highlight.ts';
import { kindsFor, type Parsed } from '#lib/palette/query.ts';
import { promptListHref } from '#lib/shared/links.ts';

export type Row = {
	/** Unique in the list: the kind and what it opens. */
	key: string;
	item: Item;
	glyph: Kind;
	/** The name or first line, matched words marked. */
	title: Segment[];
	/** What it belongs to (a prompt's session); empty when nothing. */
	context: string;
	/** When it was last active (ISO); none for a command. */
	at?: string;
};

export type Group = { kind: Kind; label: string; rows: Row[] };

export type View = {
	groups: Group[];
	/** Something was typed, nothing matched, and the way out is the one row shown. */
	noMatch: boolean;
};

const LABELS: Record<Kind, string> = {
	session: 'Sessions',
	prompt: 'Prompts',
	project: 'Projects',
	command: 'Commands',
	page: 'Not here?'
};

/** The kinds the API finds, and the key of its answer that holds each. */
const FOUND = { prompt: 'prompts', session: 'sessions', project: 'projects' } as const;
type Found = keyof typeof FOUND;

function rowOf(hit: FindHit, words: string[]): Row {
	const target = hit.target;
	const item: Item =
		target.kind === 'prompt'
			? { kind: 'prompt', id: target.id }
			: { kind: target.kind, id: target.id };
	const named = hit.title ?? (target.kind === 'session' ? 'Untitled session' : '');
	const isPrompt = target.kind === 'prompt';
	return {
		key: `${target.kind}:${target.id}`,
		item,
		glyph: target.kind,
		// A prompt is its own words, in its session; the others are their name, marked.
		title: isPrompt ? hit.snippet : highlight(named, words),
		context: isPrompt ? named : '',
		at: hit.at
	};
}

function commandRows(text: string, all?: readonly Command[]): Row[] {
	return matchCommands(text, all).map(({ command, title }) => ({
		key: `command:${command.id}`,
		item: { kind: 'command', command },
		glyph: 'command',
		title,
		context: ''
	}));
}

/** Empty, your last sessions come first; searching, the words' best prompts do. */
const order = (searching: boolean): Found[] =>
	searching ? ['prompt', 'session', 'project'] : ['session', 'prompt', 'project'];

function foundGroups(parsed: Parsed, results: FindResults): Group[] {
	const wanted = new Set<string>(kindsFor(parsed.mode));
	const words = wordsOf(parsed.text);
	return order(words.length > 0)
		.filter((kind) => wanted.has(FOUND[kind]))
		.map((kind) => ({
			kind,
			label: LABELS[kind],
			rows: results[FOUND[kind]].map((hit) => rowOf(hit, words))
		}))
		.filter((group) => group.rows.length > 0);
}

/** The way out of a search that found nothing: the Prompts page with the same words. */
function fallbackGroup(text: string): Group {
	const row: Row = {
		key: 'page:prompts',
		item: { kind: 'page', href: promptListHref({ q: text }) },
		glyph: 'page',
		title: [{ text: `Search the Prompts page for “${text}”`, hit: false }],
		context: ''
	};
	return { kind: 'page', label: LABELS.page, rows: [row] };
}

/**
 * The list for what was typed. `results` is the API's latest answer (`undefined` before the first;
 * it may be for words a moment ago), `final` that it is the answer to these words: only then can
 * "nothing matches" be said. Commands come from the registry.
 */
export function buildView(
	parsed: Parsed,
	results: FindResults | undefined,
	final: boolean,
	all?: readonly Command[]
): View {
	if (parsed.mode === 'commands') {
		const rows = commandRows(parsed.text, all);
		const groups: Group[] = rows.length ? [{ kind: 'command', label: LABELS.command, rows }] : [];
		return { groups, noMatch: false };
	}
	const groups = results ? foundGroups(parsed, results) : [];
	if (final && results && !groups.length && parsed.text) {
		return { groups: [fallbackGroup(parsed.text)], noMatch: true };
	}
	return { groups, noMatch: false };
}

/** Every row, top to bottom: what ↑ ↓ walk over. */
export const rowsOf = (groups: readonly Group[]): Row[] => groups.flatMap((g) => g.rows);

/** The index of each group's first row among all rows (what a row's position is counted from). */
export const firstIndexes = (groups: readonly Group[]): number[] =>
	groups.map((_, i) => groups.slice(0, i).reduce((n, g) => n + g.rows.length, 0));
