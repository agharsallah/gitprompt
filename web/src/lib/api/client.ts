// HTTP client for the daemon. One function per endpoint; types live in types.ts.
import type {
	Annotation,
	Catalog,
	Change,
	ConfigAction,
	Lineage,
	ProjectStory,
	Prompt,
	PromptDetail,
	SearchResults,
	SessionStory,
	Source,
	RecentCapture,
	SpecProblem,
	SpecText,
	Stats,
	Telemetry,
	Updated
} from '#lib/api/types.ts';
import { queryParams, type StatsQuery } from '#lib/api/query.ts';

type Fetch = typeof fetch;

/**
 * The daemon's error message: its `detail` string, or a list of validation errors (`{loc, msg}`).
 * Anything else, or a body that is not JSON, falls back to the HTTP status text.
 */
async function errorText(response: Response): Promise<string> {
	const detail: unknown = (await response.json().catch(() => null))?.detail;
	if (typeof detail === 'string' && detail) return detail;
	if (Array.isArray(detail) && detail.length) return detail.map(validationText).join('; ');
	return response.statusText || `HTTP ${response.status}`;
}

function validationText(entry: unknown): string {
	if (entry === null || typeof entry !== 'object') return String(entry);
	const { loc, msg } = entry as { loc?: unknown; msg?: unknown };
	const where = Array.isArray(loc) ? loc.join('.') : '';
	const what = typeof msg === 'string' ? msg : JSON.stringify(entry);
	return where ? `${where}: ${what}` : what;
}

/** The daemon answered with an error status. The message keeps the status and path first. */
export class ApiError extends Error {
	constructor(
		readonly status: number,
		readonly path: string,
		detail: string
	) {
		super(`${status} ${path}: ${detail}`);
		this.name = 'ApiError';
	}
}

async function request<T>(path: string, init?: RequestInit, fetcher: Fetch = fetch): Promise<T> {
	const response = await fetcher(path, init);
	if (!response.ok) throw new ApiError(response.status, path, await errorText(response));
	return response.json();
}

export const get = <T>(path: string, fetcher?: Fetch) => request<T>(path, undefined, fetcher);

// Writes carry X-GitPrompt: the daemon rejects writes without it (cross-site protection).
export const write = <T>(path: string, method: 'POST' | 'PUT', body?: unknown) =>
	request<T>(path, {
		method,
		headers: { 'X-GitPrompt': '1', 'Content-Type': 'application/json' },
		body: body === undefined ? undefined : JSON.stringify(body)
	});

const source = (id: string) => `/api/sources/${encodeURIComponent(id)}`;

export const api = {
	prompts: (params: URLSearchParams, fetcher?: Fetch) =>
		get<Prompt[]>(`/api/prompts?${params}`, fetcher),
	prompt: (id: number) => get<PromptDetail>(`/api/prompts/${id}`),
	annotate: (id: number, annotation: Annotation) =>
		write<Annotation>(`/api/prompts/${id}/annotation`, 'PUT', annotation),
	redact: (id: number) => write<Updated>(`/api/prompts/${id}/redact`, 'POST'),
	/** Counts, sums and response-time quantiles over every prompt the filters keep. */
	stats: (query: StatsQuery = {}, fetcher?: Fetch) =>
		get<Stats>(`/api/stats?${queryParams(query)}`, fetcher),
	/** Telemetry reported per session (lines of code, commits, slash commands) for the sessions kept. */
	telemetry: (query: StatsQuery = {}, fetcher?: Fetch) =>
		get<Telemetry>(`/api/stats/telemetry?${queryParams(query)}`, fetcher),

	/** A session as chapters (what you typed) with system markers between them. */
	sessionStory: (id: string, fetcher?: Fetch) =>
		get<SessionStory>(`/api/stories/session/${encodeURIComponent(id)}`, fetcher),
	/** A project's typed prompts in phases, with commits, cost and outcomes. */
	projectStory: (id: string, fetcher?: Fetch) =>
		get<ProjectStory>(`/api/stories/project/${encodeURIComponent(id)}`, fetcher),
	/** The versions of a prompt you typed. */
	lineage: (id: number, fetcher?: Fetch) => get<Lineage>(`/api/stories/prompt/${id}`, fetcher),
	/** Full-text search over typed text, with snippets; takes the prompt filters. */
	search: (query: StatsQuery, fetcher?: Fetch) =>
		get<SearchResults>(`/api/search?${queryParams(query)}`, fetcher),

	sources: (fetcher?: Fetch) => get<Source[]>('/api/sources', fetcher),
	/** Connect or disconnect; with `dryRun`, only returns the diff that would be written. */
	configure: (id: string, action: ConfigAction, dryRun = false) =>
		write<Change>(`${source(id)}/${action}${dryRun ? '?dry_run=true' : ''}`, 'POST'),
	remap: (id: string) => write<Updated>(`${source(id)}/remap`, 'POST'),
	/** Spec files that can't be used, with their error. */
	specProblems: (fetcher?: Fetch) => get<SpecProblem[]>('/api/sources/problems', fetcher),
	recentCaptures: (fetcher?: Fetch) => get<RecentCapture[]>('/api/sources/recent', fetcher),
	/** The YAML file a connector's spec was read from. */
	spec: (id: string) => get<SpecText>(`${source(id)}/spec`),
	/** Turn a connector on or off in GitPrompt (off: what it sends is discarded). */
	setEnabled: (id: string, enabled: boolean) =>
		write<{ enabled: boolean }>(`${source(id)}/${enabled ? 'enable' : 'disable'}`, 'POST'),

	fields: (fetcher?: Fetch) => get<Catalog>('/api/fields', fetcher)
};

/** Server-sent stream of "something was captured" notices. */
export const STREAM_URL = '/api/prompts/stream';
