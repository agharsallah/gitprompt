import type { Prompt, TurnOutcome } from '#lib/api/types.ts';

/**
 * How a turn ended, from its Stop / StopFailure / Interrupt event.
 *   done         the tool finished (Stop)
 *   failed       the turn ended on an API error (StopFailure)
 *   interrupted  you stopped the turn (Interrupt)
 *   running  no end yet, and recent enough that the tool is probably still working
 *   open     no end was recorded: interrupted, or the end was never captured
 */
export type Outcome = 'done' | 'failed' | 'interrupted' | 'running' | 'open';

export type Tone = 'neutral' | 'accent' | 'agent' | 'success' | 'warning' | 'danger' | 'info';

/** Without an end, a turn sent this recently is taken to be still running... */
export const RUNNING_WINDOW_MS = 60 * 60 * 1000;
/** ...or, when its events are known, one whose last event is this recent. */
export const ACTIVE_WINDOW_MS = 10 * 60 * 1000;

export const OUTCOMES: Record<Outcome, { label: string; tone: Tone; hint: string }> = {
	done: { label: 'Finished', tone: 'success', hint: 'The tool finished the turn.' },
	failed: { label: 'Failed', tone: 'danger', hint: 'The turn ended on an API error.' },
	interrupted: { label: 'Interrupted', tone: 'info', hint: 'You stopped the turn.' },
	running: { label: 'Working', tone: 'accent', hint: 'No end yet: the tool is still on it.' },
	open: {
		label: 'No end recorded',
		tone: 'neutral',
		hint: 'Interrupted, or its end was not captured.'
	}
};

/**
 * How the daemon groups turns (storage/store.py OUTCOME): the outcomes above, with "no end, and a
 * later prompt came" told apart as superseded (this module calls it open).
 */
export const TURN_OUTCOMES: Record<TurnOutcome, { label: string; tone: Tone; hint: string }> = {
	done: OUTCOMES.done,
	failed: OUTCOMES.failed,
	interrupted: OUTCOMES.interrupted,
	superseded: {
		label: 'Superseded',
		tone: 'warning',
		hint: 'No end was recorded, and you sent another prompt: interrupted or abandoned.'
	},
	running: OUTCOMES.running,
	open: OUTCOMES.open
};

/**
 * `superseded`: a later prompt exists in the same session, so an unfinished turn was interrupted
 * rather than still running. `lastActivity` (ms): when the turn's latest event happened, if known;
 * long agentic turns stay "running" as long as events keep arriving.
 */
export function outcome(
	p: Prompt,
	now = Date.now(),
	superseded = false,
	lastActivity?: number
): Outcome {
	const end = p.summary.ended_by;
	if (end === 'StopFailure') return 'failed';
	if (end === 'Interrupt') return 'interrupted';
	if (end === 'Stop') return 'done';
	if (superseded) return 'open';
	if (lastActivity !== undefined) return now - lastActivity < ACTIVE_WINDOW_MS ? 'running' : 'open';
	return now - Date.parse(p.received_at) < RUNNING_WINDOW_MS ? 'running' : 'open';
}

/** Ids of prompts that have a later prompt in their session, among `prompts` (any order). */
export function supersededIds(prompts: Prompt[]): Set<number> {
	const latest = new Map<string, number>();
	for (const p of prompts) {
		const s = p.fields.session_id;
		if (s && (latest.get(s) ?? -1) < p.id) latest.set(s, p.id);
	}
	return new Set(
		prompts
			.filter((p) => p.fields.session_id && latest.get(p.fields.session_id) !== p.id)
			.map((p) => p.id)
	);
}
