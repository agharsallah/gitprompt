/**
 * How a Pick up session stopped, in one line: "running · last event 2 min ago" while it goes,
 * "left 14 h ago, mid-turn" once you left it. Pure: the row, and the time.
 */
import type { PickUp, TurnOutcome } from '#lib/api/types.ts';
import { ageText, eventAge } from '#lib/sessions/running.ts';

/** Pick up shows at most this many sessions, then "All sessions ›". */
export const PICK_UP_MAX = 3;

/** How the latest turn ended, as the end of the line. A turn with no end was left mid-turn. */
const HOW: Record<TurnOutcome, string> = {
	done: 'finished',
	failed: 'failed',
	interrupted: 'interrupted',
	open: 'mid-turn',
	running: 'mid-turn',
	superseded: 'mid-turn'
};

const DAY_MS = 24 * 3_600_000;

/** "12 s", "3 min", "14 h", then "3 d" from two days on (rather than "72 h"). */
export function sinceText(ms: number): string {
	return ms >= 2 * DAY_MS ? `${Math.floor(ms / DAY_MS)} d` : ageText(ms);
}

export function stoppedText(
	row: Pick<PickUp, 'presence' | 'ended' | 'last_event_at'>,
	now: number
): string {
	const age = eventAge(row.last_event_at, now);
	if (row.presence === 'running') {
		return age === null ? 'running' : `running · last event ${ageText(age)} ago`;
	}
	const when = age === null ? 'left' : `left ${sinceText(age)} ago`;
	return `${when}, ${HOW[row.ended]}`;
}
