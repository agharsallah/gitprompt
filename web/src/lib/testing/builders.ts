/**
 * Test data builders: a valid Prompt / Event with sensible defaults, overridden per test with only
 * the fields the test is about. Deep-merges the nested objects so a test can say
 * `prompt({ summary: { cost_usd: 1 } })` without restating the rest.
 */
import type { Event, Prompt } from '#lib/api/types.ts';

let nextId = 1;

type PromptOverrides = Partial<
	Omit<Prompt, 'fields' | 'context' | 'stats' | 'summary' | 'annotation'>
> & {
	fields?: Partial<Prompt['fields']>;
	context?: Partial<Prompt['context']>;
	stats?: Partial<Prompt['stats']>;
	summary?: Partial<Prompt['summary']>;
	annotation?: Partial<Prompt['annotation']>;
};

export function prompt(overrides: PromptOverrides = {}): Prompt {
	const id = overrides.id ?? nextId++;
	return {
		id,
		source: 'claude-code',
		received_at: '2026-11-05T10:00:00.000Z',
		prompt: 'Fix the failing test',
		origin: 'typed',
		typed_text: 'prompt' in overrides ? (overrides.prompt ?? null) : 'Fix the failing test',
		injected: [],
		...overrides,
		fields: {
			session_id: 'session-a',
			prompt_id: `p-${id}`,
			cwd: '/work/app',
			...overrides.fields
		},
		context: { git_root: '/work/app', git_branch: 'main', ...overrides.context },
		stats: { words: 4, chars: 20, ...overrides.stats },
		summary: { ...overrides.summary },
		annotation: { starred: false, rating: 0, tags: [], note: '', ...overrides.annotation }
	};
}

export function event(overrides: Partial<Event> & { raw?: Event['raw'] } = {}): Event {
	return {
		id: nextId++,
		source: 'claude-code',
		received_at: '2026-11-05T10:00:10.000Z',
		event: 'PostToolUse',
		fields: {},
		raw: {},
		...overrides
	};
}

/** An ISO time `seconds` after `base`. */
export const after = (base: string, seconds: number) =>
	new Date(Date.parse(base) + seconds * 1000).toISOString();
