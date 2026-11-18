/**
 * The possible leaks as the pages say them: where a finding sits, which findings belong to a
 * prompt, the kinds they group under, when a value was hidden, what "All clear" checked. Pure,
 * tested in leaks.test.ts; the Overview panel, the reader's banners and the session story's markers
 * only lay these out.
 */
import type { StatsQuery } from '#lib/api/query.ts';
import type { Checked, LeakOut, LeakPlace, Leaks } from '#lib/api/types.ts';
import { integer, shortDate } from '#lib/shared/format.ts';
import type { PanelState } from '#lib/ui/panelState.ts';
import { PLACE_WORDS } from '#lib/leaks/hide.svelte.ts';

/** "in your prompt", "in a tool’s output". */
export const placeText = (place: LeakPlace) => `in ${PLACE_WORDS[place]}`;

/** The kind as a heading: "AWS access key", "Email address". */
export const kindTitle = (kind: string) => kind.charAt(0).toUpperCase() + kind.slice(1);

/** "Hidden on 16 Nov": the day the value was replaced (when it was found, if the day is missing). */
export const hiddenOnText = (leak: LeakOut, now?: Date) =>
	`Hidden on ${shortDate(leak.decided_at ?? leak.at, now)}`;

/** "1,204 prompts checked for 28 kinds of secret and personal data". */
export const checkedText = ({ prompts, kinds }: Checked) =>
	`${integer(prompts)} ${prompts === 1 ? 'prompt' : 'prompts'} checked for ${integer(kinds)} kinds of secret and personal data`;

/** The panel's state: the findings, or "All clear" saying what was checked. */
export const leaksState = (leaks: Leaks): PanelState =>
	leaks.open.length
		? { kind: 'ready' }
		: { kind: 'empty', title: 'All clear', message: `${checkedText(leaks.checked)}.` };

/** One kind and its findings. */
export type KindGroup = { kind: string; findings: LeakOut[] };

/** The findings grouped by kind, kinds in the order they first appear (the daemon's: newest first). */
export function groupByKind(findings: readonly LeakOut[]): KindGroup[] {
	const groups = new Map<string, LeakOut[]>();
	for (const leak of findings) {
		const group = groups.get(leak.kind);
		if (group) group.push(leak);
		else groups.set(leak.kind, [leak]);
	}
	return [...groups].map(([kind, list]) => ({ kind, findings: list }));
}

/** What a prompt shows: a banner per open finding, a quiet line per hidden one; dismissed: nothing. */
export type PromptLeaks = { open: LeakOut[]; hidden: LeakOut[] };

export function leaksOfPrompt(leaks: Leaks | null | undefined, prompt: number): PromptLeaks {
	const mine = (list: LeakOut[] = []) => list.filter((leak) => leak.prompt === prompt);
	return { open: mine(leaks?.open), hidden: mine(leaks?.hidden) };
}

/** How many open findings each prompt has: the session story's markers. */
export function openByPrompt(leaks: Leaks | null | undefined): Map<number, number> {
	const counts = new Map<number, number>();
	for (const leak of leaks?.open ?? []) counts.set(leak.prompt, (counts.get(leak.prompt) ?? 0) + 1);
	return counts;
}

/** What the reader and the session story ask for: the leaks of a session (all, without one). */
export const sessionQuery = (session: string | null | undefined): StatsQuery =>
	session ? { session } : {};
