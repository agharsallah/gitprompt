/**
 * The Sessions page's `?day=YYYY-MM-DD`: one local calendar day (a day opened from the Overview's
 * week strip), turned into the `since`/`until` filters the daemon already takes. Pure, tested in
 * day.test.ts.
 */
import { dayKey, dayLabel } from '#lib/shared/format.ts';

const DAY = /^\d{4}-\d{2}-\d{2}$/;

/** The local day named by `day`, as instants from its midnight to the next; null for no real day. */
export function dayWindow(day: string | null | undefined): { since: string; until: string } | null {
	if (!day || !DAY.test(day)) return null;
	const [y, m, d] = day.split('-').map(Number);
	const start = new Date(y, m - 1, d);
	// "2026-02-30" rolls over into March: not a day.
	if (dayKey(start) !== day) return null;
	const end = new Date(y, m - 1, d + 1);
	return { since: start.toISOString(), until: end.toISOString() };
}

/** How the list names the day: "today", "yesterday", or "on Thursday 5 November". */
export function dayPhrase(day: string, now = new Date()): string {
	const label = dayLabel(day, now);
	return label === 'Today' || label === 'Yesterday' ? label.toLowerCase() : `on ${label}`;
}
