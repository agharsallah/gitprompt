/**
 * The shape of one turn in time: tool calls (from PostToolUse / PostToolUseFailure hooks) and model
 * calls (from telemetry api_request events) as spans, in seconds from the prompt. Hooks report when
 * a tool finished plus its duration; telemetry reports when a request finished plus its duration.
 */
import type { Event } from '#lib/api/types.ts';
import { modelName } from '#lib/shared/format.ts';
import { TOOL_KINDS, toolKind, toolLabel, type ToolKind } from '#lib/insights/tools.ts';

export type TimelineRow = 'model' | ToolKind;

export type Span = {
	id: number;
	row: TimelineRow;
	label: string;
	/** What it worked on: the command, file, URL or query; or the model call's purpose. */
	detail: string;
	start: number;
	end: number;
	failed: boolean;
};

export type Timeline = {
	spans: Span[];
	rows: TimelineRow[];
	end: number;
	/** Calls that started after the turn ended (e.g. background title or summary requests). */
	after: number;
};

/** Grace after Stop before a call counts as happening after the turn. */
const AFTER_GRACE_S = 2;

const TOOL_EVENTS = new Set(['PostToolUse', 'PostToolUseFailure']);

/** When an event happened: telemetry says so (event.timestamp), hooks arrive as they happen. */
export const eventTime = (e: Event) =>
	Date.parse((e.raw['event.timestamp'] as string | undefined) ?? e.received_at);

const num = (v: unknown) => {
	const n = Number(v);
	return Number.isFinite(n) ? n : 0;
};

/** The most telling input of a tool call, as one line. */
export function toolDetail(input: unknown): string {
	if (typeof input === 'string') {
		try {
			return toolDetail(JSON.parse(input));
		} catch {
			return input;
		}
	}
	if (!input || typeof input !== 'object') return '';
	const i = input as Record<string, unknown>;
	const value =
		i.command ??
		i.file_path ??
		i.notebook_path ??
		i.path ??
		i.url ??
		i.query ??
		i.pattern ??
		i.description ??
		i.prompt;
	return typeof value === 'string' ? value.split('\n')[0] : '';
}

export function turnTimeline(events: Event[], startIso: string, responseS?: number): Timeline {
	const t0 = Date.parse(startIso);
	const spans: Span[] = [];
	for (const e of events) {
		const end = (eventTime(e) - t0) / 1000;
		const duration = num(e.raw.duration_ms) / 1000;
		if (e.event && TOOL_EVENTS.has(e.event)) {
			const name = String(e.raw.tool_name ?? e.fields.tool_name ?? 'tool');
			spans.push({
				id: e.id,
				row: toolKind(name),
				label: toolLabel(name),
				detail: toolDetail(e.raw.tool_input),
				start: Math.max(0, end - duration),
				end: Math.max(0, end),
				failed: e.event === 'PostToolUseFailure'
			});
		} else if (e.event === 'api_request') {
			const model = String(e.raw.model ?? '');
			const purpose = e.raw.query_source ? String(e.raw.query_source).replaceAll('_', ' ') : '';
			spans.push({
				id: e.id,
				row: 'model',
				label: model ? modelName(model) : 'Model call',
				detail: purpose,
				start: Math.max(0, end - duration),
				end: Math.max(0, end),
				failed: false
			});
		}
	}
	// Once the turn has ended, its timeline ends there too: later calls are only counted.
	const inside =
		responseS === undefined ? spans : spans.filter((s) => s.start <= responseS + AFTER_GRACE_S);
	inside.sort((a, b) => a.start - b.start);
	const present = new Set(inside.map((s) => s.row));
	const rows: TimelineRow[] = (['model', ...TOOL_KINDS] as const).filter((r) => present.has(r));
	const end = Math.max(responseS ?? 0, ...inside.map((s) => s.end), 1);
	return { spans: inside, rows, end, after: spans.length - inside.length };
}
