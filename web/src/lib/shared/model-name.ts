// A model id as people say it. Pure functions, unit tested in model-name.test.ts.
//
// The same model reaches us under many ids: Bedrock prefixes a region and provider
// (`us.anthropic.claude-…-v1:0`), Vertex appends a snapshot (`claude-…@20250805`), routers add a
// path (`anthropic/claude-sonnet-4.5`), Claude Code tags the context window (`claude-…[1m]`), and
// vendors date their snapshots (`-20250929`, `-2024-08-06`, `-0613`). Naming happens in three steps,
// each one small enough to read on its own:
//
//   1. unwrap  — cut what the platform added around the id.
//   2. parse   — read the dash-separated tokens into a vendor, a version and words, dropping dates.
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
	const version: string[] = [];
	const words: string[] = [];
	// The version is the first run of version tokens; numbers after it are snapshot dates.
	let run: 'before' | 'in' | 'after' = 'before';
	let versionFirst = false;

	for (const token of tokens) {
		if (run !== 'after' && isVersionPart(token, version.length === 0)) {
			if (run === 'before') versionFirst = words.length === 0;
			version.push(token);
			// A lettered version ("4o") ends the run: gpt-4o-mini is 4o, then mini.
			run = /[a-z]$/.test(token) ? 'after' : 'in';
			continue;
		}
		if (run === 'in') run = 'after';
		if (isSnapshot(token)) continue;
		if (isRevision(token)) {
			// Bedrock's -v1 is noise after a version, but the only version in claude-v2.
			if (run === 'before') {
				versionFirst = words.length === 0;
				version.push(token.slice(1));
				run = 'after';
			}
			continue;
		}
		if (!isWord(token)) return null;
		words.push(token);
	}
	return { vendor, version: version.join('.'), versionFirst, words };
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

/**
 * A piece of a version: one or two digits, dotted or not ("4", "10", "4.5"). Only the first piece
 * may end in a letter ("4o"), so gpt-4-32k is version 4 with the word 32k.
 */
const isVersionPart = (token: string, first: boolean) =>
	(first ? /^\d{1,2}(?:\.\d{1,2})*[a-z]?$/ : /^\d{1,2}(?:\.\d{1,2})*$/).test(token);

/** A snapshot rather than a name: digits (20250929, 2024, 0613, 001) or "latest". */
const isSnapshot = (token: string) => /^\d+$/.test(token) || token === 'latest';

/** Bedrock's model revision: v1, v2. */
const isRevision = (token: string) => /^v\d+$/.test(token);

/** A name word: letters and digits with at least one letter (opus, mini, 16k, 120b). */
const isWord = (token: string) => /^[a-z0-9]*[a-z][a-z0-9]*$/.test(token);
