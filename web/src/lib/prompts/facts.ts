// A prompt's facts (the daemon's `facts`, insights/facts.py) as rows, details and stories show them:
// which tool, which model, tokens and cost when reported, and why not when they aren't. Every page
// reads this one view instead of picking fields from `summary`. Pure, tested in facts.test.ts.
import type { Facts } from '#lib/api/types.ts';
import { compact, integer, modelName, money } from '#lib/shared/format.ts';
import type { Availability } from '#lib/sources/capabilities.ts';
import { names, updateNote } from '#lib/sources/capabilities.ts';

export type FactsView = {
	/** The tool's short name for a badge, and its full name for a tooltip. */
	tool: { label: string; name: string };
	/** Who answers in the tool, e.g. "Codex"; "Agent" when the spec doesn't say. */
	assistant: string;
	model: { id: string; name: string; hint: string } | null;
	tokens: { total: string; hint: string } | null;
	/** null when the tool reported no cost: say nothing, never "$0". */
	cost: string | null;
	permissionMode: string | null;
};

const MODEL_HINT = {
	usage: 'Model of most model calls in this turn',
	hook: 'Model the tool reported with the prompt'
};

export function factsView(f: Facts): FactsView {
	const t = f.tokens;
	return {
		tool: { label: f.connector.label, name: f.connector.name },
		assistant: f.connector.assistant || 'Agent',
		model: f.model
			? {
					id: f.model,
					name: modelName(f.model),
					hint: `${MODEL_HINT[f.model_source ?? 'hook']}: ${f.model}`
				}
			: null,
		tokens: t
			? {
					total: compact(t.total),
					hint:
						`${integer(t.total)} tokens: ${integer(t.input)} input, ${integer(t.output)} output` +
						(t.cache_read ? `, ${integer(t.cache_read)} read from the cache` : '') +
						(t.reasoning ? ` (${integer(t.reasoning)} reasoning)` : '')
				}
			: null,
		cost: f.cost_usd == null ? null : money(f.cost_usd),
		permissionMode: f.permission_mode
	};
}

/**
 * Why a turn shows no tokens, from what its tool can report (`availability` of `usage` among the
 * tool's layers): needs a newer version, needs a layer connected, or can't. Null when it shows them.
 */
export function missingTokens(f: Facts, usage: Availability | undefined): string | null {
	if (f.tokens) return null;
	if (!usage) return null;
	if (usage.updatable.length) return updateNote(usage);
	if (usage.connectable.length)
		return `Connect ${usage.connectable[0].layer} to see tokens and cost.`;
	if (usage.available.length) return 'No model call was reported for this turn.';
	return `${names(usage.unavailable) || f.connector.name} doesn't report tokens.`;
}
