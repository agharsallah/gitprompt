// Who produced a prompt, as the dashboard shows it. The daemon classifies (insights/origin.py,
// ADR-0010); this module only names and draws each origin, in one place. Pure, tested in origin.test.ts.
import {
	BellRing,
	Bot,
	CircleHelp,
	Keyboard,
	MessagesSquare,
	SquareSlash,
	Workflow,
	type LucideIcon
} from '@lucide/svelte';
import type { Injected, Origin, Prompt } from '#lib/api/types.ts';
import { integer, plural } from '#lib/shared/format.ts';

export type OriginInfo = {
	label: string;
	icon: LucideIcon;
	/** Not typed by the user: shown muted, with this icon and label (never colour alone). */
	system: boolean;
	/** One sentence for a tooltip. */
	hint: string;
};

export const ORIGINS: Record<Origin, OriginInfo> = {
	typed: { label: 'Typed', icon: Keyboard, system: false, hint: 'You typed this prompt.' },
	slash_command: {
		label: 'Slash command',
		icon: SquareSlash,
		system: false,
		hint: 'You ran a slash command.'
	},
	task_notification: {
		label: 'Task notification',
		icon: BellRing,
		system: true,
		hint: 'Added by the tool when a background task finished.'
	},
	agent_message: {
		label: 'Agent message',
		icon: MessagesSquare,
		system: true,
		hint: 'Sent by another agent, not typed by you.'
	},
	subagent: {
		label: 'Subagent',
		icon: Bot,
		system: true,
		hint: 'Written by the model for a subagent.'
	},
	programmatic: {
		label: 'Programmatic',
		icon: Workflow,
		system: true,
		hint: 'Sent by a program (a headless run or the Agent SDK), not an interactive session.'
	},
	unknown: {
		label: 'Unknown origin',
		icon: CircleHelp,
		system: true,
		hint: 'No text was typed, or there is no rule for this source.'
	}
};

/** Older daemons send no origin: treat those prompts as typed. */
export const originOf = (p: Pick<Prompt, 'origin'>): OriginInfo =>
	ORIGINS[p.origin] ?? ORIGINS.typed;

export const isSystem = (p: Pick<Prompt, 'origin'>) => originOf(p).system;

/** The text to show as the prompt: what was typed, without the context the tool added. */
export const typedText = (p: Pick<Prompt, 'prompt' | 'typed_text'>): string | null =>
	p.typed_text || p.prompt;

const KINDS: Record<string, string> = {
	ide_selection: 'IDE selection',
	ide_opened_file: 'Open file',
	ide_diagnostics: 'IDE diagnostics',
	'system-reminder': 'System reminder'
};

/** "IDE selection", or the tag in words for kinds without a name here. */
export function injectedLabel(kind: string): string {
	if (KINDS[kind]) return KINDS[kind];
	const words = kind.replace(/[-_:]+/g, ' ').trim();
	return words.charAt(0).toUpperCase() + words.slice(1);
}

/** "IDE selection, 2,241 characters" / "2 blocks, 3,000 characters", or '' when nothing was added. */
export function injectedSummary(injected: Injected[]): string {
	if (!injected.length) return '';
	const kinds = [...new Set(injected.map((i) => i.kind))];
	const what =
		kinds.length === 1 && injected.length === 1
			? injectedLabel(kinds[0])
			: plural(injected.length, 'block');
	const chars = injected.reduce((n, i) => n + i.chars, 0);
	return `${what}, ${integer(chars)} ${chars === 1 ? 'character' : 'characters'}`;
}
