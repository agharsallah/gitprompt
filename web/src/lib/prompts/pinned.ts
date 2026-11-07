// The server-side filters a link can set on the Prompts page besides search, tag and origin (the
// Overview's doorways: a day, an hour, a tool, an outcome...), as removable chips. Pure, tested in
// pinned.test.ts.
import type { Filters, Has, TurnOutcome } from '#lib/api/types.ts';
import { TURN_OUTCOMES } from '#lib/insights/outcome.ts';
import { bands, SIZES } from '#lib/insights/series.ts';
import { querySourceLabel } from '#lib/insights/series.ts';
import { asKind, KIND_INFO } from '#lib/insights/tools.ts';
import { weekdays } from '#lib/insights/week.ts';
import { folder, modelName } from '#lib/shared/format.ts';

export const HAS_LABELS: Record<Has, string> = {
	code: 'Has a code block',
	url: 'Has a link',
	mention: 'Has an @-mention',
	secret: 'Possible secret',
	commit: 'Agent committed',
	subagent: 'Used a subagent',
	tool_failure: 'A tool failed',
	api_error: 'API error',
	denial: 'Denied or rejected tool',
	compaction: 'Compacted',
	no_follow_up: 'No follow-up'
};

export type Lists = { model: string[]; surface: string[]; outcome: TurnOutcome[]; has: Has[] };
export type Pin = { key: string; value: string; label: string };

const instant = (iso: string) =>
	new Date(iso).toLocaleString('en-GB', {
		day: 'numeric',
		month: 'short',
		hour: '2-digit',
		minute: '2-digit'
	});

const pad = (n: number) => String(n).padStart(2, '0');

const LABELS: Partial<Record<keyof Filters, (value: string) => string>> = {
	session: (v) => `Session ${v.slice(0, 8)}`,
	repo: (v) => folder(v) ?? v,
	source: (v) => `Source ${v}`,
	since: (v) => `From ${instant(v)}`,
	until: (v) => `Before ${instant(v)}`,
	weekday: (v) => weekdays('monday')[Number(v) - 1]?.plural ?? `Weekday ${v}`,
	hour: (v) => `${pad(Number(v))}:00 to ${pad((Number(v) + 1) % 24)}:00`,
	command: (v) => v,
	tool: (v) => `Tool ${v}`,
	tool_kind: (v) => KIND_INFO[asKind(v)].label,
	query_source: (v) => `Model calls: ${querySourceLabel(v)}`,
	size: (v) => `${bands([], SIZES, String).find((b) => b.key === v)?.label ?? v} characters`,
	before: () => 'Older prompts'
};

/** Every pinned filter, in a stable order, each with a label to show on its chip. */
export function pins(filters: Filters, lists: Lists): Pin[] {
	const out: Pin[] = [];
	for (const [key, label] of Object.entries(LABELS) as [keyof Filters, (v: string) => string][]) {
		const value = filters[key];
		if (value) out.push({ key, value, label: label(value) });
	}
	for (const m of lists.model) out.push({ key: 'model', value: m, label: modelName(m) });
	for (const s of lists.surface) out.push({ key: 'surface', value: s, label: `Typed in ${s}` });
	for (const o of lists.outcome)
		out.push({ key: 'outcome', value: o, label: TURN_OUTCOMES[o]?.label ?? o });
	for (const h of lists.has) out.push({ key: 'has', value: h, label: HAS_LABELS[h] ?? h });
	return out;
}

/** A query string without one pinned value (and without the selection). */
export function unpin(search: string, pin: Pin): string {
	const params = new URLSearchParams(search);
	const kept = params.getAll(pin.key).filter((v) => v !== pin.value);
	params.delete(pin.key);
	for (const v of kept) params.append(pin.key, v);
	params.delete('id');
	return params.toString();
}
