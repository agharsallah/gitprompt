// Types mirror the FastAPI response models (src/gitprompt/api/routes/*.py, insights/catalog.py,
// sources/connection.py). The derived shapes (TurnSummary, PromptStats, SessionFacts) mirror
// insights/derivations.py; GitContext mirrors core/git.py. Every derived value is optional: the
// daemon leaves out what it could not derive (compact()), and older prompts may lack newer values.
export type Meta = Record<string, unknown>;

export type Annotation = { starred: boolean; rating: -1 | 0 | 1; tags: string[]; note: string };

/** Fields the source spec maps (sources/bundled/*.yaml). */
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

/** Repository and tool state observed when the prompt was received (core/git.py). */
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

/** derivations.PROMPT_STATS */
export type PromptStats = {
	chars?: number;
	words?: number;
	lines?: number;
	approx_tokens?: number;
	slash_command?: string;
	at_mentions?: string[];
	urls?: string[];
	code_blocks?: number;
	questions?: number;
	secrets?: string[];
};

/** derivations.TURN: what happened while the tool worked on one prompt. */
export type TurnSummary = {
	turn?: number;
	since_previous_prompt_s?: number;
	response_s?: number;
	ended_by?: 'Stop' | 'StopFailure' | string;
	error?: unknown;
	response?: string;
	tool_calls?: Record<string, number>;
	tool_failures?: number;
	tool_time_s?: number;
	files_written?: string[];
	files_read?: string[];
	bash_commands?: string[];
	web?: string[];
	mcp_tools?: string[];
	slash_command?: string;
	subagents?: Record<string, number>;
	tasks?: string[];
	notifications?: Record<string, number>;
	permission_denied?: string[];
	compactions?: number;
	model_switches?: string[];
	api_requests?: number;
	models?: Record<string, number>;
	input_tokens?: number;
	output_tokens?: number;
	cache_read_tokens?: number;
	cache_creation_tokens?: number;
	cost_usd?: number;
	api_time_s?: number;
	api_errors?: number;
	tool_rejections?: number;
};

/** derivations.SESSION: facts about the whole session. */
export type SessionFacts = {
	events?: Record<string, number>;
	first_event_at?: string;
	last_event_at?: string;
	ended?: string;
	instructions_loaded?: string[];
	cwd_changes?: string[];
	config_changes?: string[];
	metric_totals?: Record<string, number>;
	environment?: Record<string, unknown>;
};

export type Prompt = {
	id: number;
	source: string;
	received_at: string;
	prompt: string | null;
	fields: PromptFields & Meta; // mapped by the source spec
	context: GitContext & Meta; // observed at ingest: git state, tool version
	annotation: Annotation;
	stats: PromptStats & Meta; // derived from the prompt text
	summary: TurnSummary & Meta; // derived from the prompt's events
};

export type Event = {
	id: number;
	source: string;
	received_at: string;
	event: string | null;
	fields: Meta;
	raw: Meta;
};

export type Commit = { hash: string; subject: string; author: string; time: string };

export type PromptDetail = Prompt & {
	raw: Meta;
	session: SessionFacts & Meta;
	commits: { commits?: Commit[]; diffstat?: string; until?: string };
	events: Event[];
};

export type Source = {
	id: string;
	name: string;
	detected: boolean;
	version: string | null;
	connected: boolean;
	outdated: boolean;
	config_file: string;
	terms: string;
	blocks: string[];
	warnings: string[];
	total: number;
	unmapped: number;
	events: number;
	last_at: string | null;
};

/** A change to a tool's config file: previewed (dry run) before it is applied. */
export type Change = { file: string; diff: string; applied: boolean; backup: string | null };

/** Source actions that change the tool's config file, so they go through the consent flow. */
export type ConfigAction = 'connect' | 'disconnect';

export type Updated = { updated: number };

export type Field = {
	source: string;
	event: string;
	path: string;
	count: number;
	sample: unknown;
	documented: boolean;
};

export type Catalog = {
	fields: Field[];
	not_captured: { source: string; event: string; reason: string }[];
	derived: Record<string, string>;
};

/** Prompt list filters (query parameters of GET /api/prompts). */
export const FILTER_KEYS = ['q', 'tag', 'starred', 'session', 'repo', 'source', 'before'] as const;
export type Filters = Partial<Record<(typeof FILTER_KEYS)[number], string>>;
