// The daemon's API types.
//
// Everything the daemon sends or accepts is GENERATED from the Rust wire types into generated.ts
// (`just types`; `cargo test` fails while it is stale). Never copy a wire type here by hand: change
// the Rust struct, regenerate, and import it from this module as before.
//
// This file only adds what the client defines itself: the filters and queries it builds, and
// narrower views of the open-ended JSON maps (a prompt's `fields` and `context`).
import type * as Wire from '#lib/api/generated.ts';

export type * from '#lib/api/generated.ts';

// --- open-ended maps, as the dashboard reads them ----------------------------------------------

export type Meta = Record<string, unknown>;

/** Fields the source spec maps (backend/crates/gitprompt-core/specs/*.yaml); others may follow. */
export type PromptFields = {
	event?: string;
	prompt?: string;
	session_id?: string;
	prompt_id?: string;
	cwd?: string;
	transcript_path?: string;
	permission_mode?: string;
	session_title?: string;
};

/** Repository and tool state observed when the prompt was received (gitprompt-core git.rs). */
export type GitContext = {
	git_root?: string;
	git_worktree?: boolean;
	git_branch?: string;
	git_detached?: boolean;
	git_head?: string;
	git_upstream?: string;
	git_ahead?: number;
	git_behind?: number;
	git_staged?: number;
	git_modified?: number;
	git_untracked?: number;
	git_conflicts?: number;
	git_remote?: string;
	git_repo?: string;
	git_host?: string;
	git_head_subject?: string;
	git_head_time?: string;
	git_head_author?: string;
	tool_version?: string;
};

type Narrowed<T> = Omit<T, 'fields' | 'context'> & {
	fields: PromptFields & Meta;
	context: GitContext & Meta;
};

export type Prompt = Narrowed<Wire.Prompt>;
export type PromptDetail = Narrowed<Wire.PromptDetail>;

// --- views derived from the wire types ------------------------------------------------------

/** One run of a word diff: the op and its text. */
export type DiffOp = NonNullable<Wire.Version['diff']>[number];

export type MarkerGroup = Extract<Wire.StoryItem, { kind: 'markers' }>;

// --- what the client builds ---------------------------------------------------------------

/** Source actions that change the tool's config file, so they go through the consent flow. */
export type ConfigAction = 'connect' | 'disconnect';

/** What a prompt or its turn contains (`has`, gitprompt-store filter.rs). */
export type Has =
	| 'code'
	| 'url'
	| 'mention'
	| 'secret'
	| 'commit'
	| 'subagent'
	| 'tool_failure'
	| 'api_error'
	| 'denial'
	| 'compaction'
	| 'no_follow_up';

/**
 * Single-valued prompt filters the Prompts page keeps in its URL (gitprompt-store PromptFilter).
 * `origin`, `model`, `outcome` and `has` take several values and are handled on their own;
 * `since`/`until` are ISO-8601 instants.
 */
export const FILTER_KEYS = [
	'q',
	'tag',
	'starred',
	'session',
	'repo',
	'source',
	'since',
	'until',
	'weekday',
	'hour',
	'command',
	'tool',
	'tool_kind',
	'query_source',
	'size',
	'before'
] as const;
export type Filters = Partial<Record<(typeof FILTER_KEYS)[number], string>>;

/** The multi-valued filters. */
export const LIST_KEYS = ['origin', 'model', 'surface', 'outcome', 'has'] as const;

/** Every filter GET /api/prompts and GET /api/stats accept. Empty lists mean "any". */
export type PromptQuery = Omit<Filters, 'session'> & {
	session?: string | string[];
	origin?: Wire.Origin[];
	model?: string[];
	/** Typed in one of these apps (Surface ids). */
	surface?: string[];
	outcome?: Wire.TurnOutcome[];
	has?: Has[];
};
