/**
 * Test data builders: a valid Prompt / Event with sensible defaults, overridden per test with only
 * the fields the test is about. Deep-merges the nested objects so a test can say
 * `prompt({ summary: { cost_usd: 1 } })` without restating the rest.
 */
import type { Event, Prompt, Source, StatsBucket } from '#lib/api/types.ts';

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

/** A connector as GET /api/sources reports it: installed, not connected, nothing received. */
export function source(overrides: Partial<Source> = {}): Source {
	return {
		id: 'claude-code',
		name: 'Claude Code',
		part_of: null,
		layer: 'Hooks',
		summary: 'Your prompts, the tools each turn used and how the turn ended.',
		detected: true,
		version: '2.1.316',
		connected: false,
		outdated: false,
		config_file: '/Users/me/.claude/settings.json',
		terms: 'Uses the tool’s documented hooks in your own user settings.',
		blocks: [],
		warnings: [],
		total: 0,
		unmapped: 0,
		events: 0,
		last_at: null,
		hourly: Array(24).fill(0),
		collects: ['UserPromptSubmit', 'PostToolUse', 'Stop'],
		not_stored: {},
		not_collected: { PreToolUse: 'A decision point: GitPrompt stays out of decisions.' },
		enabled: true,
		capabilities: ['prompt', 'cwd', 'turn_end', 'answer', 'tools', 'subagents', 'origin_markers'],
		join: 'prompt_id',
		spec: {
			name: 'claude-code.yaml',
			path: '/gitprompt/sources/bundled/claude-code.yaml',
			origin: 'bundled',
			overrides: false
		},
		spec_problem: null,
		...overrides
	};
}

/** An ISO time `seconds` after `base`. */
export const after = (base: string, seconds: number) =>
	new Date(Date.parse(base) + seconds * 1000).toISOString();

/** A /api/stats bucket: zeros everywhere except what the test sets. */
export function bucket(overrides: Partial<StatsBucket> = {}): StatsBucket {
	return {
		key: null,
		detail: null,
		prompts: 0,
		sessions: 0,
		api_requests: 0,
		cost_usd: 0,
		input_tokens: 0,
		output_tokens: 0,
		cache_read_tokens: 0,
		cache_creation_tokens: 0,
		api_time_s: 0,
		api_errors: 0,
		tool_calls: 0,
		tool_failures: 0,
		tool_time_s: 0,
		subagent_tool_calls: 0,
		subagents: 0,
		files_written: 0,
		git_commits: 0,
		compactions: 0,
		denials: 0,
		rejections: 0,
		ended: 0,
		failed: 0,
		followed: 0,
		with_code: 0,
		with_url: 0,
		with_mention: 0,
		with_secret: 0,
		with_commit: 0,
		distinct_texts: 0,
		response_s_p50: null,
		response_s_p90: null,
		first_at: null,
		last_at: null,
		...overrides
	};
}
