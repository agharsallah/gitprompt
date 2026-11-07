// HTTP client for the daemon. One function per endpoint; types live in types.ts.
import type {
	Annotation,
	Catalog,
	Change,
	ConfigAction,
	Prompt,
	PromptDetail,
	Source,
	SpecProblem,
	SpecText,
	Stats,
	Telemetry,
	Updated
} from '#lib/api/types.ts';
import { queryParams, type StatsQuery } from '#lib/api/query.ts';

type Fetch = typeof fetch;

/** The daemon's error message: `detail` from FastAPI (a string, or a list of validation errors). */
async function errorText(response: Response): Promise<string> {
	const detail = (await response.json().catch(() => null))?.detail;
	if (Array.isArray(detail)) return detail.map((d) => `${d.loc?.join('.')}: ${d.msg}`).join('; ');
	return detail ?? response.statusText;
}

async function request<T>(path: string, init?: RequestInit, fetcher: Fetch = fetch): Promise<T> {
	const response = await fetcher(path, init);
	if (!response.ok) throw new Error(`${response.status} ${path}: ${await errorText(response)}`);
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

	sources: (fetcher?: Fetch) => get<Source[]>('/api/sources', fetcher),
	/** Connect or disconnect; with `dryRun`, only returns the diff that would be written. */
	configure: (id: string, action: ConfigAction, dryRun = false) =>
		write<Change>(`${source(id)}/${action}${dryRun ? '?dry_run=true' : ''}`, 'POST'),
	remap: (id: string) => write<Updated>(`${source(id)}/remap`, 'POST'),
	/** Spec files that can't be used, with their error. */
	specProblems: (fetcher?: Fetch) => get<SpecProblem[]>('/api/sources/problems', fetcher),
	/** The YAML file a connector's spec was read from. */
	spec: (id: string) => get<SpecText>(`${source(id)}/spec`),
	/** Turn a connector on or off in GitPrompt (off: what it sends is discarded). */
	setEnabled: (id: string, enabled: boolean) =>
		write<{ enabled: boolean }>(`${source(id)}/${enabled ? 'enable' : 'disable'}`, 'POST'),

	fields: (fetcher?: Fetch) => get<Catalog>('/api/fields', fetcher)
};

/** Server-sent stream of "something was captured" notices. */
export const STREAM_URL = '/api/prompts/stream';
