/** The time windows the insight pages offer, carried in the URL as `?range=`. */
export const RANGES = [
	{ id: '24h', label: '24 hours', short: '24h', days: 1 },
	{ id: '7d', label: '7 days', short: '7d', days: 7 },
	{ id: '30d', label: '30 days', short: '30d', days: 30 },
	{ id: 'all', label: 'All time', short: 'All', days: null }
] as const;

export type RangeId = (typeof RANGES)[number]['id'];
export type Range = (typeof RANGES)[number];

export const DEFAULT_RANGE: RangeId = '7d';

export function parseRange(value: string | null | undefined): Range {
	return RANGES.find((r) => r.id === value) ?? RANGES.find((r) => r.id === DEFAULT_RANGE)!;
}

/** Start of the window: `days` before now (null for all time). */
export function rangeStart(range: Range, now = Date.now()): Date | null {
	return range.days == null ? null : new Date(now - range.days * 86_400_000);
}

/** How the window reads in a sentence: "in the last 7 days", "in the last 24 hours", "so far". */
export function rangePhrase(range: Range): string {
	return range.days == null ? 'so far' : `in the last ${range.label}`;
}
