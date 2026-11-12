/**
 * Presentation helpers for prompt stories (the daemon builds them: insights/stories.py, ADR-0016).
 * Links, labels and sentences only. Pure, tested in story.test.ts.
 */
import { Bot, Minimize2, type LucideIcon } from '@lucide/svelte';
import type { Diffstat, Marker, MarkerKind, Phase, TurnOutcome } from '#lib/api/types.ts';
import type { Outcome } from '#lib/insights/outcome.ts';
import { ORIGINS } from '#lib/prompts/origin.ts';
import { duration, integer, plural, shortDate, timeOfDay } from '#lib/shared/format.ts';

/**
 * A project's id in story URLs: the base64url (no padding) of its git root or folder, as the daemon
 * computes it (stories.project_id), so a link needs no lookup.
 */
export function projectId(repo: string): string {
	let binary = '';
	for (const byte of new TextEncoder().encode(repo)) binary += String.fromCharCode(byte);
	return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

/** The element id of a prompt in a story, for "show in story" links (`#prompt-12`). */
export const anchor = (id: number) => `prompt-${id}`;

/** The rail node for a turn outcome: superseded and open both draw an open node. */
export const railOutcome = (o: TurnOutcome): Outcome => (o === 'superseded' ? 'open' : o);

export type MarkerInfo = { label: string; plural: string; icon: LucideIcon; hint: string };

/** How each kind of system marker is named and drawn (icon and label, never colour alone). */
export const MARKERS: Record<MarkerKind, MarkerInfo> = {
	task_notification: { ...ORIGINS.task_notification, plural: 'task notifications' },
	agent_message: { ...ORIGINS.agent_message, plural: 'agent messages' },
	subagent: { ...ORIGINS.subagent, plural: 'subagent prompts' },
	programmatic: { ...ORIGINS.programmatic, plural: 'programmatic prompts' },
	unknown: { ...ORIGINS.unknown, plural: 'prompts of unknown origin' },
	subagent_prompt: {
		label: 'Subagent started',
		plural: 'subagents started',
		icon: Bot,
		hint: 'The model wrote a brief for a subagent. Not typed by you.'
	},
	compaction: {
		label: 'Context compacted',
		plural: 'compactions',
		icon: Minimize2,
		hint: 'The tool summarized the conversation to free context.'
	}
};

/** "3 task notifications, 1 subagent started": what a folded group of markers holds. */
export function markerSummary(markers: Pick<Marker, 'kind'>[]): string {
	const counts = new Map<MarkerKind, number>();
	for (const m of markers) counts.set(m.kind, (counts.get(m.kind) ?? 0) + 1);
	return [...counts]
		.map(([kind, n]) =>
			n === 1 ? `1 ${MARKERS[kind].label.toLowerCase()}` : `${integer(n)} ${MARKERS[kind].plural}`
		)
		.join(', ');
}

/** "+120 −8 in 4 files", or '' without changes. The signs carry the meaning, not colour. */
export function diffstatText(d: Pick<Diffstat, 'files' | 'insertions' | 'deletions'> | null) {
	if (!d || !(d.files || d.insertions || d.deletions)) return '';
	return `+${integer(d.insertions)} −${integer(d.deletions)} in ${plural(d.files, 'file')}`;
}

/** Why a phase began, in words. */
export function phaseReason(phase: Pick<Phase, 'reason' | 'gap_s' | 'branches'>): string {
	if (phase.reason === 'gap') return `After a ${duration(phase.gap_s ?? 0)} pause`;
	if (phase.reason === 'branch') return `Moved to ${phase.branches[0] ?? 'another branch'}`;
	return 'First prompts';
}

/** Outcome order for mixes and legends: best first. */
export const OUTCOME_ORDER: TurnOutcome[] = [
	'done',
	'interrupted',
	'superseded',
	'open',
	'failed',
	'running'
];

/** The colour token of each outcome in mixes, the same as the rail and the turn strip. */
export const OUTCOME_COLORS: Record<TurnOutcome, string> = {
	done: 'var(--color-accent)',
	interrupted: 'var(--color-info-solid)',
	superseded: 'var(--color-warning-solid)',
	open: 'var(--color-border-control)',
	failed: 'var(--color-danger-solid)',
	running: 'var(--color-agent)'
};

/** Items of a long list to render now: the first `page` times `pages`, at least one page. */
export const visible = <T>(items: T[], pages: number, page: number): T[] =>
	items.slice(0, Math.max(1, pages) * page);

/** "5 Nov, 10:23 to 18:40", or "5 Nov, 10:23 to 6 Nov, 15:43" over several days. */
export function span(first: string | null, last: string | null): string {
	if (!first) return '';
	const start = `${shortDate(first)}, ${timeOfDay(first)}`;
	if (!last || last === first) return start;
	const sameDay = new Date(first).toDateString() === new Date(last).toDateString();
	return `${start} to ${sameDay ? '' : `${shortDate(last)}, `}${timeOfDay(last)}`;
}
