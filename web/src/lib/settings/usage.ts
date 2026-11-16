/** Settings, Developer: the usage counts of the last 30 days, and of the last 7 among them. */
import type { UsageOut } from '#lib/api/types.ts';
import { USAGE_ACTIONS } from '#lib/shared/usage-actions.ts';

export type UsageRow = { action: string; week: number; month: number };

/** `day` ("2026-01-05") moved by `by` days, on the calendar (no time zone, no clock). */
export function shiftDay(day: string, by: number): string {
	const [y, m, d] = day.split('-').map(Number);
	return new Date(Date.UTC(y, m - 1, d + by)).toISOString().slice(0, 10);
}

/**
 * One row per action, the last 7 days (the range's last week, `until` included) and all of it
 * (the range asked for). Every known action is listed, so one never used shows as 0; most used
 * first, then by name.
 */
export function usageRows(out: UsageOut): UsageRow[] {
	const weekStart = shiftDay(out.until, -6);
	const rows = new Map<string, UsageRow>(
		USAGE_ACTIONS.map((action) => [action, { action, week: 0, month: 0 }])
	);
	for (const { action, day, n } of out.counts) {
		const row = rows.get(action) ?? { action, week: 0, month: 0 };
		row.month += n;
		if (day >= weekStart) row.week += n;
		rows.set(action, row);
	}
	return [...rows.values()].sort(
		(a, b) => b.month - a.month || b.week - a.week || a.action.localeCompare(b.action)
	);
}
