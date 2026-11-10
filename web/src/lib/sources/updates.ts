// Connections that need an update ("Update available"), as the app-wide hint in the sidebar shows
// them. Pure, tested in updates.test.ts; the hint itself is layout/UpdateHint.svelte.
import type { Source } from '#lib/api/types.ts';
import { connectQuery } from '#lib/sources/setup.ts';
import { connectorState, layerName, tools } from '#lib/sources/state.ts';

/** One connection to update: its connector id and the name a person knows it by. */
export type PendingUpdate = { id: string; name: string };

/**
 * Every connector whose state is `outdated`, named by its tool ("Claude Code"), plus the layer when
 * it is one ("Claude Code · Telemetry"). Only `outdated`: a disabled or blocked connector can't
 * be updated, so it isn't asked for.
 */
export function pendingUpdates(sources: Source[], now: number): PendingUpdate[] {
	return tools(sources).flatMap((tool) =>
		tool.layers
			.filter((s) => connectorState(s, now) === 'outdated')
			.map((s) => ({
				id: s.id,
				name: s.id === tool.id ? tool.name : `${tool.name} · ${layerName(s)}`
			}))
	);
}

/** The query for the hint's link to Connectors: open the review of the only update, else none. */
export const updateQuery = (updates: PendingUpdate[]) =>
	updates.length === 1 ? connectQuery(updates[0].id) : '';

/** The hint's one line: what needs an update. */
export function updateSummary(updates: PendingUpdate[]): string {
	if (updates.length === 1) return `${updates[0].name} can capture more after an update.`;
	if (updates.length === 2) return `${updates[0].name} and ${updates[1].name} can capture more.`;
	return `${updates.length} connections can capture more after an update.`;
}

/** localStorage key of a dismissed hint. It stores the set it dismissed, so a new update shows again. */
export const DISMISS_KEY = 'gitprompt-update-hint';

/** The value stored when this set of updates is dismissed. */
export const dismissValue = (updates: PendingUpdate[]) =>
	updates
		.map((u) => u.id)
		.sort()
		.join(',');

/** Was this set dismissed? Only while it is unchanged or smaller: one more update brings the hint back. */
export function isDismissed(updates: PendingUpdate[], stored: string | null): boolean {
	if (!stored || !updates.length) return false;
	const dismissed = new Set(stored.split(','));
	return updates.every((u) => dismissed.has(u.id));
}
