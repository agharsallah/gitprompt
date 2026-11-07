/** How the Settings page explains values: where a setting comes from, what a clean-up removes. */
import type { CleanupReport, Setting } from '#lib/api/settings.ts';
import type { Tone } from '#lib/insights/outcome.ts';

export type Origin = { label: string; tone: Tone; detail: string };

/** Where a setting's value comes from, and what that means for changing it. */
export function origin(setting: Setting, file = 'preferences.json'): Origin {
	switch (setting.source) {
		case 'env':
			return {
				label: `Set by ${setting.env}`,
				tone: 'warning',
				detail: `Environment variables win over the settings file. Unset ${setting.env} and restart GitPrompt to change this here.`
			};
		case 'file':
			return { label: 'Saved', tone: 'info', detail: `Saved in ${file}.` };
		default:
			return { label: 'Default', tone: 'neutral', detail: 'Not changed: the built-in default.' };
	}
}

export function bytes(n: number): string {
	if (n < 1_000) return `${n} B`;
	if (n < 1_000_000) return `${(n / 1_000).toFixed(0)} kB`;
	if (n < 1_000_000_000) return `${(n / 1_000_000).toFixed(1)} MB`;
	return `${(n / 1_000_000_000).toFixed(2)} GB`;
}

const WHY: Record<CleanupReport['removals'][number]['why'], string> = {
	'dropped by spec': 'no longer kept by the source spec',
	expired: 'older than the retention period',
	selected: 'matches your selection'
};

export const reason = (why: keyof typeof WHY) => WHY[why];

/** One line about a clean-up: what it would remove (dry run) or removed (applied). */
export function summary(report: CleanupReport): string {
	const events = `${report.rows.toLocaleString('en')} event${report.rows === 1 ? '' : 's'}`;
	if (!report.applied) {
		return report.rows
			? `Would delete ${events} (about ${bytes(report.bytes)}). Prompts and notes are kept.`
			: 'Nothing to delete.';
	}
	const after = report.size_after ?? report.size_before;
	return `Deleted ${events}. The database went from ${bytes(report.size_before)} to ${bytes(after)}.`;
}

/** The start of a local calendar day ("2026-10-01") as an ISO time, for "received before". */
export function startOfDay(date: string): string | null {
	const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(date);
	if (!match) return null;
	const [, y, m, d] = match.map(Number);
	return new Date(y, m - 1, d).toISOString();
}

/** The time `days` days before `now`, as an ISO time (the retention cutoff). */
export const daysAgo = (days: number, now = Date.now()) =>
	new Date(now - days * 86_400_000).toISOString();
