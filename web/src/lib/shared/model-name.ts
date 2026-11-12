// A model id as people say it. Pure functions, unit tested in model-name.test.ts.
//
// The same model reaches us under many ids: Bedrock prefixes a region and provider
// (`us.anthropic.claude-…-v1:0`), Vertex appends a snapshot (`claude-…@20250805`), routers add a
// path (`anthropic/claude-sonnet-4.5`), Claude Code tags the context window (`claude-…[1m]`), and
// vendors date their snapshots (`-20250929`, `-2024-08-06`, `-0613`). Naming happens in three steps,
// each one small enough to read on its own:
//
//   1. unwrap  — cut what the platform added around the id.
//   2. parse   — split the dash-separated tokens into a vendor, the version (findVersion) and the
//                name words around it (nameWords), dropping dates.
//   3. scheme  — let the vendor's naming scheme (SCHEMES) put them together.
//
// Names come from the id, never from a list of known models, so tomorrow's models name themselves.
// An id no scheme can read passes through unchanged.

/** What a model id says, once wrappers and snapshot dates are gone. */
export interface ModelParts {
	/** The first token: "claude", "gpt", "gemini". */
	vendor: string;
	/** Dotted version: "4.5" (from 4-5 or 4.5), "5.1", "4o". Empty when the id has none. */
	version: string;
	/** The version came straight after the vendor (gpt-5-codex, claude-3-opus), before any word. */
	versionFirst: boolean;
	/** The remaining words in order: family, variants, sizes ("opus", "codex", "mini", "16k"). */
	words: string[];
}

/** Turns an id's parts into a name, or null when the id doesn't follow the vendor's scheme. */
type Scheme = (parts: ModelParts) => string | null;

const capital = (word: string) => word.charAt(0).toUpperCase() + word.slice(1);
const spaced = (...parts: string[]) => parts.filter(Boolean).join(' ');

/**
 * One naming scheme per vendor, keyed by the id's first token. To name a new vendor's models, add
 * an entry: it receives the parsed parts and returns the name. (A Map, so an id like
 * "constructor-1" can't reach Object.prototype.)
 */
export const SCHEMES: ReadonlyMap<string, Scheme> = new Map<string, Scheme>([
	// claude-opus-5-5 -> Opus 5.5; claude-3-5-sonnet -> Sonnet 3.5; claude-v2 -> Claude 2.
	// The family word names the model, wherever the version sits.
	[
		'claude',
		({ version, words: [family, ...variants] }) =>
			spaced(family ? capital(family) : 'Claude', version, ...variants.map(capital))
	],
	// gpt-5.1-codex-mini -> GPT-5.1 Codex Mini; the version is part of the name.
	[
		'gpt',
		({ version, versionFirst, words }) =>
			version && versionFirst ? spaced(`GPT-${version}`, ...words.map(capital)) : null
	],
	// gemini-2.5-flash-lite -> Gemini 2.5 Flash Lite.
	[
		'gemini',
		({ version, versionFirst, words }) =>
			version && versionFirst ? spaced('Gemini', version, ...words.map(capital)) : null
	]
]);

/**
 * A model id as people say it: "claude-opus-5-5" -> "Opus 5.5", "claude-sonnet-4-5-20250929" ->
 * "Sonnet 4.5", "gpt-6-luna" -> "GPT-6 Luna", "gpt-4o-2024-08-06" -> "GPT-4o". Unknown ids pass
 * through.
 */
export function modelName(id: string): string {
	const parts = parseModelId(id);
	return (parts && SCHEMES.get(parts.vendor)?.(parts)) || id;
}

/** The parts of a model id, or null when a token is neither a version, a date nor a word. */
export function parseModelId(id: string): ModelParts | null {
	const [vendor = '', ...tokens] = unwrap(id).split('-').filter(Boolean);
	const { start, end, version } = findVersion(tokens);
	const before = nameWords(tokens.slice(0, start));
	const after = nameWords(tokens.slice(end));
	if (!before || !after) return null;
	const versionFirst = version !== '' && before.length === 0;
	return { vendor, version, versionFirst, words: [...before, ...after] };
}

/** Where the version sits among an id's tokens (`tokens.slice(start, end)`), and how it reads. */
interface VersionSpan {
	start: number;
	end: number;
	/** Dotted: "4.5", "4o", "2"; empty when the id has none (then start = end = tokens.length). */
	version: string;
}

/**
 * The version: the first run of version pieces ("4", "5" -> 4.5), which a lettered piece ends
 * ("4o": gpt-4o-mini is 4o, then mini). Bedrock's revision is the version only when it comes before
 * any piece (claude-v2 -> 2); later numbers are snapshot dates, which nameWords drops.
 */
function findVersion(tokens: readonly string[]): VersionSpan {
	const start = tokens.findIndex((t) => isVersionHead(t) || isRevision(t));
	if (start < 0) return { start: tokens.length, end: tokens.length, version: '' };
	const head = tokens[start];
	if (isRevision(head)) return { start, end: start + 1, version: head.slice(1) };
	let end = start + 1;
	if (!/[a-z]$/.test(head)) while (end < tokens.length && isVersionTail(tokens[end])) end++;
	return { start, end, version: tokens.slice(start, end).join('.') };
}

/**
 * The name words among the tokens around the version, without snapshot dates and Bedrock revisions
 * (noise next to a version); null when a token is none of these, so the id is not ours to name.
 */
function nameWords(tokens: readonly string[]): string[] | null {
	const kept = tokens.filter((t) => !isSnapshot(t) && !isRevision(t));
	return kept.every(isWord) ? kept : null;
}

/**
 * The id without what a platform wrapped around it, lowercased:
 * "us.anthropic.claude-opus-5-5-v1:0" -> "claude-opus-5-5-v1".
 */
function unwrap(id: string): string {
	let s = id.trim().toLowerCase();
	// A router or API path: anthropic/claude-…, openai/gpt-5, models/gemini-2.5-pro.
	s = s.slice(s.lastIndexOf('/') + 1);
	// Whatever follows the id itself: [1m] (context window), @20250805 (Vertex), :0 (Bedrock).
	const tail = s.search(/[[@:]/);
	if (tail >= 0) s = s.slice(0, tail);
	// Bedrock's region and provider, dot-separated: us.anthropic.claude-…. A dot inside the id itself
	// follows a digit (gpt-5.1, gemini-2.5), so only all-letter segments are prefixes.
	for (let dot = s.indexOf('.'); dot > 0 && isLetters(s.slice(0, dot)); dot = s.indexOf('.')) {
		s = s.slice(dot + 1);
	}
	return s;
}

const isLetters = (s: string) => /^[a-z]+$/.test(s);

/** The first piece of a version: one or two digits, dotted or not ("4", "10", "4.5"), or "4o". */
const isVersionHead = (token: string) => /^\d{1,2}(?:\.\d{1,2})*[a-z]?$/.test(token);

/** A later piece: digits only, so gpt-4-32k is version 4 with the word 32k. */
const isVersionTail = (token: string) => /^\d{1,2}(?:\.\d{1,2})*$/.test(token);

/** A snapshot rather than a name: digits (20250929, 2024, 0613, 001) or "latest". */
const isSnapshot = (token: string) => /^\d+$/.test(token) || token === 'latest';

/** Bedrock's model revision: v1, v2. */
const isRevision = (token: string) => /^v\d+$/.test(token);

/** A name word: letters and digits with at least one letter (opus, mini, 16k, 120b). */
const isWord = (token: string) => /^[a-z0-9]*[a-z][a-z0-9]*$/.test(token);
