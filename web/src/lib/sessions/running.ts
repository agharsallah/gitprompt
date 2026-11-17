/**
 * Is a session still going? Pure: what the page knows (when the session last did anything, whether
 * its latest turn has an end, what the live stream is doing, the time) gives `running` or `left`.
 * The page feeds it from the shared `clock` and `live` (RunningNow.svelte), so a session flips to
 * `left` on the clock's next tick once its last event is old enough.
 */
import type { StreamState } from '#lib/api/live.svelte.ts';
import { ACTIVE_WINDOW_MS } from '#lib/insights/outcome.ts';
import type { Prompt, TurnOutcome } from '#lib/api/types.ts';

/**
 * A session whose last event is younger than this, and whose latest turn has no end, is running.
 * The same window that makes a turn "Working" on the prompt rows (insights/outcome.ts), so a card
 * and its rows never disagree.
 */
export const RUNNING_THRESHOLD_MS = ACTIVE_WINDOW_MS;

export type Presence = 'running' | 'left';

/** What tells a running session from one you left. */
export type Activity = {
	/** When the session's latest event or prompt arrived (ISO-8601). */
	lastEventAt?: string | null;
	/** Its latest turn has an end (finished, failed or interrupted): the tool is waiting for you. */
	ended?: boolean;
};

/**
 * The stream can bring a session's next event while it is `open` (or about to be). Otherwise the
 * page would claim "running" about a session it can no longer watch.
 */
const WATCHING: readonly StreamState[] = ['open', 'connecting'];

/** Milliseconds since the last event (never negative: the daemon's clock and the page's differ), or null when unknown. */
export function eventAge(lastEventAt: string | null | undefined, now: number): number | null {
	const at = lastEventAt ? Date.parse(lastEventAt) : NaN;
	return Number.isFinite(at) ? Math.max(0, now - at) : null;
}

export function presence(activity: Activity, now: number, stream: StreamState): Presence {
	if (activity.ended || !WATCHING.includes(stream)) return 'left';
	const age = eventAge(activity.lastEventAt, now);
	return age !== null && age < RUNNING_THRESHOLD_MS ? 'running' : 'left';
}

/** "12 s", "3 min", "2 h": the age as the running line writes it. */
export function ageText(ms: number): string {
	const s = Math.floor(ms / 1000);
	if (s < 60) return `${s} s`;
	const m = Math.floor(s / 60);
	return m < 60 ? `${m} min` : `${Math.floor(m / 60)} h`;
}

/** "Running · last event 12 s ago". */
export const runningLine = (ageMs: number) => `Running · last event ${ageText(ageMs)} ago`;

const ENDED: readonly TurnOutcome[] = ['done', 'failed', 'interrupted'];

/** A turn's outcome that means the tool is done with it (a story chapter's). */
export const endedOutcome = (outcome: TurnOutcome | undefined) =>
	!!outcome && ENDED.includes(outcome);

/** The latest prompt of a session carries its end (Stop, StopFailure or Interrupt). */
export const endedPrompt = (prompts: readonly Prompt[]) => !!prompts.at(-1)?.summary.ended_by;
