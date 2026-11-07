/**
 * Stories told from a list of prompts the page already loaded (the values are derived on read by
 * the daemon; this only groups and sums them): a session's card, and a prompt's tokens. Statistics
 * over many prompts come from GET /api/stats instead (lib/insights/overview.ts). Pure functions,
 * tested in aggregate.test.ts.
 */
import type { Prompt } from '#lib/api/types.ts';
import { mergeProject, projectOf, type ProjectRef } from '#lib/insights/project.ts';

export const sum = (values: number[]) => values.reduce((a, b) => a + b, 0);

const n = (v: number | undefined | null) => (typeof v === 'number' && Number.isFinite(v) ? v : 0);

/** Sum of a `{name: count}` map. */
const total = (m: Record<string, number> | undefined) => sum(Object.values(m ?? {}));

// ---- tokens ------------------------------------------------------------------------------

export type Tokens = { input: number; output: number; cacheRead: number; cacheWrite: number };

export function tokensOf(p: Prompt): Tokens {
	const s = p.summary;
	return {
		input: n(s.input_tokens),
		output: n(s.output_tokens),
		cacheRead: n(s.cache_read_tokens),
		cacheWrite: n(s.cache_creation_tokens)
	};
}

export function addTokens(a: Tokens, b: Tokens): Tokens {
	return {
		input: a.input + b.input,
		output: a.output + b.output,
		cacheRead: a.cacheRead + b.cacheRead,
		cacheWrite: a.cacheWrite + b.cacheWrite
	};
}

export const tokenTotal = (t: Tokens) => t.input + t.output + t.cacheRead + t.cacheWrite;

/** Share of the prompt-side tokens served from cache (0..1), null without telemetry. */
export function cacheShare(t: Tokens): number | null {
	const prompt = t.input + t.cacheRead + t.cacheWrite;
	return prompt ? t.cacheRead / prompt : null;
}

const ZERO: Tokens = { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 };

// ---- sessions ----------------------------------------------------------------------------

export type SessionSummary = {
	id: string;
	project: ProjectRef;
	/** Oldest first, as the conversation went. */
	prompts: Prompt[];
	title: string | null;
	branch: string | null;
	startedAt: string;
	/** When the last turn ended (or was sent, if it has no end). */
	endedAt: string;
	cost: number;
	tokens: Tokens;
	failed: number;
	toolCalls: number;
	filesWritten: number;
	models: string[];
	permissionModes: string[];
};

const endOf = (p: Prompt) =>
	new Date(Date.parse(p.received_at) + n(p.summary.response_s) * 1000).toISOString();

/** Prompts grouped by session, most recently active first. */
export function sessions(prompts: Prompt[]): SessionSummary[] {
	const groups = new Map<string, Prompt[]>();
	for (const p of prompts) {
		const id = p.fields.session_id ?? `prompt-${p.id}`;
		groups.set(id, [...(groups.get(id) ?? []), p]);
	}
	return [...groups]
		.map(([id, list]) => {
			const ordered = [...list].sort((a, b) => a.id - b.id);
			const first = ordered[0];
			const last = ordered.at(-1)!;
			const files = new Set(ordered.flatMap((p) => p.summary.files_written ?? []));
			const models = new Set(ordered.flatMap((p) => Object.keys(p.summary.models ?? {})));
			const modes = new Set(
				ordered.map((p) => p.fields.permission_mode).filter((m): m is string => !!m)
			);
			return {
				id,
				project: ordered.map(projectOf).reduce(mergeProject),
				prompts: ordered,
				title:
					[...ordered].reverse().find((p) => p.fields.session_title)?.fields.session_title ?? null,
				branch: last.context.git_branch ?? null,
				startedAt: first.received_at,
				endedAt: endOf(last),
				cost: sum(ordered.map((p) => n(p.summary.cost_usd))),
				tokens: ordered.map(tokensOf).reduce(addTokens, ZERO),
				failed: ordered.filter((p) => p.summary.ended_by === 'StopFailure').length,
				toolCalls: sum(ordered.map((p) => total(p.summary.tool_calls))),
				filesWritten: files.size,
				models: [...models],
				permissionModes: [...modes]
			};
		})
		.sort((a, b) => Date.parse(b.endedAt) - Date.parse(a.endedAt));
}
