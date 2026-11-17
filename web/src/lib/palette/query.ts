/**
 * What was typed in the palette. A first character narrows without a mode switch: `>` the commands,
 * `#` the projects, `@` the sessions; otherwise everything is searched.
 */
import type { FindKind } from '#lib/api/client.ts';

export type Mode = 'all' | 'commands' | 'projects' | 'sessions';
export type Parsed = { mode: Mode; text: string };

const PREFIXES: Record<string, Mode> = { '>': 'commands', '#': 'projects', '@': 'sessions' };

/** The mode a first character picks, and the words after it. */
export function parseQuery(raw: string): Parsed {
	const trimmed = raw.trimStart();
	const mode = PREFIXES[trimmed.charAt(0)];
	return mode ? { mode, text: trimmed.slice(1).trim() } : { mode: 'all', text: raw.trim() };
}

/** What to ask the API for in a mode: every kind, or the one the prefix names; none for commands. */
export function kindsFor(mode: Mode): FindKind[] {
	if (mode === 'projects') return ['projects'];
	if (mode === 'sessions') return ['sessions'];
	return mode === 'commands' ? [] : ['prompts', 'sessions', 'projects'];
}
