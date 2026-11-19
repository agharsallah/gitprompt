/**
 * Test data builders: a valid Prompt / Event with sensible defaults, overridden per test with only
 * the fields the test is about. Deep-merges the nested objects so a test can say
 * `prompt({ summary: { cost_usd: 1 } })` without restating the rest.
 */
import type {
	Attention,
	Delta,
	Event,
	Facts,
	KeyChanges,
	LeakOut,
	Leaks,
	PickUp,
	Prompt,
	RedonePart,
	RedoneWork,
	SessionStory,
	Source,
	Stats,
	StatsBucket,
	WorthItem
} from '#lib/api/types.ts';
import type { PreferenceKey, Preferences, Setting, SettingsState } from '#lib/api/settings.ts';
import type { Overview } from '#lib/insights/overview.ts';
import { parseRange, rangeStart } from '#lib/insights/range.ts';

let nextId = 1;

type PromptOverrides = Partial<
	Omit<Prompt, 'fields' | 'context' | 'stats' | 'summary' | 'annotation' | 'facts'>
> & {
	facts?: Partial<Facts>;
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
		annotation: { starred: false, rating: 0, tags: [], note: '', ...overrides.annotation },
		facts: facts(overrides.facts)
	};
}

/** A prompt's facts: a finished Claude Code turn with no usage reported. */
export function facts(overrides: Partial<Facts> = {}): Facts {
	return {
		connector: {
			id: 'claude-code',
			name: 'Claude Code',
			label: 'Claude Code',
			assistant: 'Claude'
		},
		surface: null,
		model: null,
		models: [],
		model_source: null,
		tokens: null,
		cost_usd: null,
		api_requests: 0,
		outcome: 'done',
		permission_mode: null,
		tool_version: null,
		response_s: null,
		answered: false,
		tool_calls: 0,
		tool_failures: 0,
		files_written: 0,
		git_commits: 0,
		subagents: 0,
		...overrides
	};
}

export function event(overrides: Partial<Event> & { raw?: Event['raw'] } = {}): Event {
	return {
		id: nextId++,
		source: 'claude-code',
		received_at: '2026-11-05T10:00:10.000Z',
		event: 'PostToolUse',
		session_id: null,
		prompt_id: null,
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
		assistant: 'Claude',
		part_of: null,
		layer: 'Hooks',
		summary: 'Your prompts, the tools each turn used and how the turn ended.',
		detected: true,
		version: '2.1.316',
		connected: false,
		outdated: false,
		config_file: '/Users/me/.claude/settings.json',
		terms: 'Uses the tool’s documented hooks in your own user settings.',
		after_connect: '',
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
		needs_version: {},
		join: 'prompt_id',
		captures: [],
		cannot_capture: {},
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
		agent_time_s: 0,
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
		span_s: null,
		first_at: null,
		last_at: null,
		parts: null,
		...overrides
	};
}

/** GET /api/stats without grouping: these totals, nothing else. */
export function stats(overrides: Partial<Stats> = {}): Stats {
	return { group_by: null, split_by: null, totals: bucket(), groups: [], ...overrides };
}

/**
 * A day (or "YYYY-MM-DD HH" hour) of `group_by=day&split_by=repo`: its prompts per project, the
 * day's totals summed from them. `null` as a project is "no project".
 */
export function splitDay(
	key: string,
	projects: [string | null, number][],
	overrides: Partial<StatsBucket> = {}
): StatsBucket {
	const parts = projects.map(([repo, prompts]) => bucket({ key: repo, prompts }));
	return bucket({ key, prompts: parts.reduce((n, p) => n + p.prompts, 0), parts, ...overrides });
}

/**
 * Everything the Overview loads, empty: the range `7d` ending at `now`, every stats call with no
 * groups; the test sets what it is about (e.g. `strip`).
 */
export function overview(overrides: Partial<Overview> = {}, now = Date.now()): Overview {
	const filters = overrides.filters ?? { range: parseRange('7d'), origins: [] };
	const empty = stats();
	return {
		filters,
		query: {},
		now,
		start: rangeStart(filters.range, now),
		hourly: filters.range.id === '24h',
		activity: empty,
		strip: empty,
		keyFigures: empty,
		week: empty,
		sessionLengths: empty,
		origins: empty,
		commands: empty,
		sizes: empty,
		texts: empty,
		models: empty,
		work: empty,
		querySources: empty,
		repos: empty,
		outcomes: empty,
		toolKinds: empty,
		tools: empty,
		telemetry: { sessions: 0, metrics: {}, commands: [] },
		options: { source: empty, surface: empty, repo: empty, model: empty },
		starred: [],
		attention: attention(),
		leaks: leaks(),
		...overrides
	};
}

/** One figure now and before; the change is worked out as the daemon does (absent without one). */
export function delta(now: number, before?: number): Delta {
	if (before === undefined) return { now };
	return before ? { now, before, change: (now - before) / before } : { now, before };
}

/** The key figures compared: all flat at zero except what the test sets. */
export function changes(overrides: Partial<KeyChanges> = {}): KeyChanges {
	const flat = delta(0, 0);
	return {
		prompts: flat,
		sessions: flat,
		cost_usd: flat,
		agent_time_s: flat,
		git_commits: flat,
		...overrides
	};
}

const part = (overrides: Partial<RedonePart> = {}): RedonePart => ({
	turns: 0,
	agent_time_s: 0,
	...overrides
});

/** What the redone turns cost: nothing redone, unless the test says what. */
export function redone(
	overrides: Partial<Omit<RedoneWork, 'failed' | 'interrupted' | 'reworded'>> & {
		failed?: Partial<RedonePart>;
		interrupted?: Partial<RedonePart>;
		reworded?: Partial<RedonePart>;
	} = {}
): RedoneWork {
	return {
		turns: 0,
		agent_time_s: 0,
		...overrides,
		failed: part(overrides.failed),
		interrupted: part(overrides.interrupted),
		reworded: part(overrides.reworded)
	};
}

/** GET /api/stories/session/{id}: an empty Claude Code session; the test sets what it is about. */
export function sessionStory(overrides: Partial<SessionStory> = {}): SessionStory {
	return {
		session: 'session-a',
		label: 'Fix the failing test',
		source: 'claude-code',
		connector: {
			id: 'claude-code',
			name: 'Claude Code',
			label: 'Claude Code',
			assistant: 'Claude'
		},
		surface: null,
		models: {},
		project: null,
		branches: [],
		started_at: '2026-11-05T10:00:00Z',
		ended_at: '2026-11-05T10:05:00Z',
		totals: {
			chapters: 0,
			markers: 0,
			cost_usd: 0,
			tool_calls: 0,
			files_written: 0,
			outcomes: {},
			retries: 0,
			diffstat: null
		},
		items: [],
		...overrides
	};
}

/** GET /api/settings with these values, each a default the daemon applies at once. */
export function settingsState(values: Preferences): SettingsState {
	const setting = <K extends PreferenceKey>(key: K): Setting<K> => ({
		value: values[key],
		default: values[key],
		source: 'default',
		env: `GITPROMPT_${key.toUpperCase()}`,
		locked: false,
		applies: 'now'
	});
	return {
		file: '/home/dev/.gitprompt/preferences.json',
		settings: {
			default_range: setting('default_range'),
			week_start: setting('week_start'),
			event_retention_days: setting('event_retention_days'),
			hide_secrets_on_ingest: setting('hide_secrets_on_ingest'),
			disabled_sources: setting('disabled_sources'),
			recall_shortcut: setting('recall_shortcut'),
			leak_allow: setting('leak_allow'),
			private_terms: setting('private_terms'),
			notify_long_turn: setting('notify_long_turn'),
			notify_long_turn_minutes: setting('notify_long_turn_minutes'),
			notify_leaks: setting('notify_leaks'),
			notify_quiet_connector: setting('notify_quiet_connector'),
			notify_quiet_connector_minutes: setting('notify_quiet_connector_minutes'),
			recap: setting('recap'),
			recap_weekday: setting('recap_weekday'),
			recap_hour: setting('recap_hour')
		}
	};
}

/** One Pick up row: a Claude Code session you left after a finished turn, with no resume command. */
export function pickUp(overrides: Partial<PickUp> = {}): PickUp {
	return {
		session: 'session-a',
		label: 'Fix the failing test',
		project: 'app',
		presence: 'left',
		ended: 'done',
		last_event_at: '2026-11-05T10:00:00Z',
		...overrides
	};
}

/** One Worth a look row: a reused prompt, with its text to copy as a command. */
export function worthItem(overrides: Partial<WorthItem> = {}): WorthItem {
	return {
		kind: 'repeated_prompt',
		text: 'You typed “run the tests and fix what fails” 6 times.',
		action: { type: 'copy_command', name: 'run-tests', text: 'run the tests and fix what fails' },
		target: { page: 'prompts', q: 'run the tests and fix what fails' },
		...overrides
	};
}

/** GET /api/attention: nothing needs you, unless the test says what does. */
export function attention(overrides: Partial<Attention> = {}): Attention {
	const pick_up = overrides.pick_up ?? [];
	const worth_a_look = overrides.worth_a_look ?? [];
	return {
		pick_up,
		worth_a_look,
		nothing_needs_you: pick_up.length === 0 && worth_a_look.length === 0,
		...overrides
	};
}

/** One open finding of GET /api/leaks: an email address in a prompt. */
export function leakOut(overrides: Partial<LeakOut> = {}): LeakOut {
	return {
		id: 'f1',
		kind: 'email',
		masked: 'j•••@example.com',
		place: 'prompt',
		prompt: 1,
		at: '2026-11-05T10:00:00Z',
		...overrides
	};
}

/** GET /api/leaks: nothing found among 10 prompts, unless the test says what was. */
export function leaks(overrides: Partial<Leaks> = {}): Leaks {
	return {
		open: [],
		hidden: [],
		dismissed: [],
		checked: { prompts: 10, kinds: 28 },
		...overrides
	};
}
