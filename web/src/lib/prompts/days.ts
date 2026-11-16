/**
 * Prompts grouped by the day they arrived, newest day first. The list comes in the server's order
 * (newest id first), and ids follow arrival only roughly: history imported later, or a backfill,
 * puts an old day after a newer one. So a day is one group however its prompts are spread through
 * the list, and each group keeps the list's order inside it.
 */
import type { Prompt } from '#lib/api/types.ts';
import { dayKey } from '#lib/shared/format.ts';

export type Day = { key: string; prompts: Prompt[] };

/** One group per day (a `YYYY-MM-DD` key), newest first; prompts with no date come last. */
export function byDay(prompts: readonly Prompt[]): Day[] {
	const groups = new Map<string, Prompt[]>();
	for (const p of prompts) {
		const key = dayKey(p.received_at);
		const group = groups.get(key);
		if (group) group.push(p);
		else groups.set(key, [p]);
	}
	return [...groups]
		.map(([key, list]) => ({ key, prompts: list }))
		.sort((a, b) => dayOrder(b.key) - dayOrder(a.key) || (a.key < b.key ? 1 : -1));
}

/** Dated keys before the undated one; among dated keys, the string order is the date order. */
const dayOrder = (key: string) => (/^\d{4}-\d{2}-\d{2}$/.test(key) ? 1 : 0);
