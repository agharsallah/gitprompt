// Display helpers shared by components. Pure functions, unit tested in format.test.ts.
//
// Every helper accepts what the API can send (null, NaN, a date that doesn't parse) and shows a
// dash or a neutral value, never "NaN", "-0" or "Invalid Date".
import type { Meta } from '#lib/api/types.ts';

export { modelName } from '#lib/shared/model-name.ts';

/** Shown in place of a value that is missing or can't be read. */
const DASH = '—';

const MINUTE = 60;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;
const WEEK = 7 * DAY;

/** A Date for an ISO string or Date, or null when there is none or it doesn't parse. */
function toDate(value: Date | string | null | undefined): Date | null {
	if (value == null || value === '') return null;
	const date = value instanceof Date ? value : new Date(value);
	return Number.isNaN(date.getTime()) ? null : date;
}

/** A number, or a string holding one; null for anything else. */
function toNumber(value: unknown): number | null {
	const n =
		typeof value === 'number' ? value : typeof value === 'string' && value.trim() ? +value : NaN;
	return Number.isFinite(n) ? n : null;
}

// ---------------------------------------------------------------------------------------------
// Values

/** Keep only the keys of `data` listed in `keys` (or, with `except`, all other keys). */
export const pick = (data: Meta, keys: readonly string[], except = false) =>
	Object.fromEntries(
		Object.entries(data).filter(([key]) => (except ? !keys.includes(key) : keys.includes(key)))
	);

/** Last segment of a path, POSIX or Windows; null for anything else or a path with no segment. */
export const folder = (path: unknown): string | null =>
	typeof path === 'string' ? (path.split(/[/\\]/).filter(Boolean).at(-1) ?? null) : null;

/** Items in a list, or the sum of a `{name: count}` map; values that aren't numbers count 0. */
export function count(value: unknown): number {
	if (Array.isArray(value)) return value.length;
	if (value === null || typeof value !== 'object') return 0;
	return Object.values(value).reduce((sum: number, n) => sum + (toNumber(n) ?? 0), 0);
}

/** The items that are there: a list the daemon may pad with nulls (an event without the value). */
export const present = <T>(items: readonly (T | null)[] | undefined): T[] =>
	(items ?? []).filter((item): item is T => item != null);

/** One-line rendering of any JSON value. */
export const inline = (value: unknown): string =>
	typeof value === 'string' ? value : (JSON.stringify(value) ?? String(value));

// ---------------------------------------------------------------------------------------------
// Numbers

// `signDisplay: 'negative'` keeps a value that rounds to zero from showing as "-0".
const integerFormat = new Intl.NumberFormat('en', {
	maximumFractionDigits: 0,
	signDisplay: 'negative'
});
const compactFormat = new Intl.NumberFormat('en', {
	notation: 'compact',
	maximumFractionDigits: 1,
	signDisplay: 'negative'
});
const oneDecimal = new Intl.NumberFormat('en', {
	maximumFractionDigits: 1,
	signDisplay: 'negative'
});
const centsFormat = new Intl.NumberFormat('en', {
	style: 'currency',
	currency: 'USD',
	signDisplay: 'negative'
});
const dollarsFormat = new Intl.NumberFormat('en', {
	style: 'currency',
	currency: 'USD',
	maximumFractionDigits: 0,
	minimumFractionDigits: 0,
	signDisplay: 'negative'
});

/** Below this, `compact` writes the number in full. */
const COMPACT_FROM = 10_000;

/** 1,284 under ten thousand, 12.9K / 4.2M above. */
export const compact = (n: number) =>
	!Number.isFinite(n)
		? DASH
		: Math.abs(n) < COMPACT_FROM
			? integerFormat.format(n)
			: compactFormat.format(n);

/** Full integer with separators, for tables and tooltips. */
export const integer = (n: number) => (Number.isFinite(n) ? integerFormat.format(n) : DASH);

/** From this many dollars, `money` drops the cents. */
const WHOLE_DOLLARS_FROM = 100;

/** Dollars: cents under $100, whole dollars above, "<$0.01" for crumbs, "$0" for nothing. */
export function money(usd: number | null | undefined): string {
	if (usd == null || usd === 0) return '$0';
	if (!Number.isFinite(usd)) return DASH;
	const size = Math.abs(usd);
	if (size < 0.01) return usd > 0 ? '<$0.01' : '>-$0.01';
	// Compare the amount as shown: $99.996 shows as $100.00, so it is past the threshold.
	const cents = Math.round(size * 100) / 100;
	return (cents < WHOLE_DOLLARS_FROM ? centsFormat : dollarsFormat).format(usd);
}

/** A ratio (0..1) as a whole percentage; under 1% shows one decimal so it isn't read as zero. */
export function percent(ratio: number | null | undefined): string {
	if (ratio == null || !Number.isFinite(ratio)) return DASH;
	const p = ratio * 100;
	if (p > 0) {
		const tenths = Math.round(p * 10) / 10;
		if (tenths < 0.1) return '<0.1%';
		if (tenths < 1) return `${oneDecimal.format(tenths)}%`;
	}
	return `${integerFormat.format(p)}%`;
}

/** "1 prompt", "3 prompts"; irregular plurals pass `many`. The word agrees with the number shown. */
export function plural(n: number, one: string, many = `${one}s`): string {
	const shown = integer(n);
	return `${shown} ${shown === '1' ? one : many}`;
}

// ---------------------------------------------------------------------------------------------
// Durations

const pad2 = (n: number) => String(n).padStart(2, '0');

/** Seconds as 4.2s, 42s, 3m 05s, 1h 12m, 2d 3h. */
export function duration(seconds: number | null | undefined): string {
	if (seconds == null || !Number.isFinite(seconds)) return DASH;
	const s = Math.max(0, seconds);
	if (s < 10) return `${oneDecimal.format(s)}s`;
	// Round once, then split: 3599.6s is 1h 00m, not 59m 60s.
	const total = Math.round(s);
	if (total < MINUTE) return `${total}s`;
	const minutes = Math.floor(total / MINUTE);
	if (total < HOUR) return `${minutes}m ${pad2(total % MINUTE)}s`;
	const hours = Math.floor(total / HOUR);
	if (total < DAY) return `${hours}h ${pad2(minutes % 60)}m`;
	return `${Math.floor(total / DAY)}d ${hours % 24}h`;
}

/** Units for axis ticks, largest first. */
const TICK_UNITS = [
	['d', DAY],
	['h', HOUR],
	['m', MINUTE],
	['s', 1]
] as const;

/** A duration for an axis tick, in its largest unit with at most one decimal: 30s, 1.5m, 2h, 1d. */
export function shortDuration(seconds: number): string {
	if (!Number.isFinite(seconds)) return DASH;
	if (seconds === 0) return '0';
	const size = Math.abs(seconds);
	const tenths = (length: number) => Math.round((size / length) * 10) / 10;
	// The largest unit the tick reaches once rounded: 59.99s is 1m, not 60s.
	const [unit, length] = TICK_UNITS.find(([, length]) => tenths(length) >= 1) ?? ['s', 1];
	return `${seconds < 0 ? '-' : ''}${oneDecimal.format(tenths(length))}${unit}`;
}

// ---------------------------------------------------------------------------------------------
// Dates and times

/** Date and time in the viewer's locale. */
export const when = (iso: string | null | undefined) => toDate(iso)?.toLocaleString() ?? DASH;

const relativeFormat = new Intl.RelativeTimeFormat('en', { numeric: 'auto', style: 'long' });

/** Closer than this is "just now". */
const JUST_NOW = 45;

/** Each unit `relative` counts in, and how many of it before the next unit takes over. */
const RELATIVE_UNITS = [
	['minute', MINUTE, 60],
	['hour', HOUR, 24],
	['day', DAY, 7]
] as const satisfies readonly (readonly [Intl.RelativeTimeFormatUnit, number, number])[];

/** "just now", "5 minutes ago", "yesterday", "in 3 hours", then a short date from a week away. */
export function relative(iso: string | null | undefined, now = Date.now()): string {
	if (!iso) return 'never';
	const date = toDate(iso);
	if (!date) return DASH;
	const s = (date.getTime() - now) / 1000;
	const size = Math.abs(s);
	if (size < JUST_NOW) return 'just now';
	for (const [unit, length, limit] of RELATIVE_UNITS) {
		// Counted as shown: 59.6 minutes is "1 hour ago", not "60 minutes ago".
		const n = Math.round(size / length);
		if (n < limit) return relativeFormat.format(Math.sign(s) * n, unit);
	}
	return shortDate(date, new Date(now));
}

/** "5 Nov", or "5 Nov 2025" outside the current year. */
export function shortDate(iso: Date | string, now = new Date()): string {
	const date = toDate(iso);
	if (!date) return DASH;
	return date.toLocaleDateString('en-GB', {
		day: 'numeric',
		month: 'short',
		year: date.getFullYear() === now.getFullYear() ? undefined : 'numeric'
	});
}

/** "14:05" in the viewer's locale. */
export const timeOfDay = (iso: string) =>
	toDate(iso)?.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }) ?? DASH;

/** Local calendar day as YYYY-MM-DD: the key for grouping by day. A date that doesn't parse is "—". */
export function dayKey(date: Date | string): string {
	const d = toDate(date);
	if (!d) return DASH;
	return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`;
}

/** "Today", "Yesterday", or "Thursday 5 November" for a YYYY-MM-DD key; any other key as given. */
export function dayLabel(key: string, now = new Date()): string {
	if (key === dayKey(now)) return 'Today';
	const yesterday = new Date(now);
	yesterday.setDate(now.getDate() - 1);
	if (key === dayKey(yesterday)) return 'Yesterday';
	const [y, m, d] = key.split('-').map(Number);
	const date = new Date(y, m - 1, d);
	// Not a real day key ("2026-02-30", "—", ""): the date rolled over or didn't parse.
	if (!toDate(date) || dayKey(date) !== key) return key;
	return date.toLocaleDateString('en-GB', {
		weekday: 'long',
		day: 'numeric',
		month: 'long',
		year: y === now.getFullYear() ? undefined : 'numeric'
	});
}

// ---------------------------------------------------------------------------------------------
// Text

/** A cut closer to the start than this share of `max` loses too much: cut mid-word instead. */
const WORD_CUT_FROM = 0.6;

/**
 * A text on one line, trimmed to `max` characters (plus "…") on a word boundary. For a prompt,
 * pass its typed text (prompts/origin.ts typedText): the daemon has already removed the context a
 * tool added.
 */
export function excerpt(text: string | null | undefined, max = 140): string {
	if (!text) return '';
	const line = text.trim().replace(/\s+/g, ' ');
	if (line.length <= max) return line;
	let cut = line.slice(0, max);
	const space = cut.lastIndexOf(' ');
	if (space > max * WORD_CUT_FROM) cut = cut.slice(0, space);
	// Don't leave half of an emoji (a lone high surrogate) before the ellipsis.
	else if (/[\uD800-\uDBFF]$/.test(cut)) cut = cut.slice(0, -1);
	return `${cut.trimEnd()}…`;
}

/** Where people's home folders live: /Users/<name> on macOS, /home/<name> on Linux. */
const HOME_PARENTS = ['/Users/', '/home/'];
/** Folders under HOME_PARENTS that aren't anyone's home. */
const NOT_HOMES = new Set(['Shared']);

/** A path shown relative to a root it lives under (the repository), else from "~", else as given. */
export function relativePath(path: string, root?: string | null): string {
	const base = root?.replace(/\/+$/, '');
	if (base && path.startsWith(`${base}/`)) return path.slice(base.length + 1);
	for (const parent of HOME_PARENTS) {
		if (!path.startsWith(parent)) continue;
		const end = path.indexOf('/', parent.length);
		const user = end < 0 ? path.slice(parent.length) : path.slice(parent.length, end);
		if (user && !NOT_HOMES.has(user)) return end < 0 ? '~' : `~${path.slice(end)}`;
	}
	return path;
}

/**
 * The words in a key, split at separators (_ . - [ ] spaces) and case changes: "HTTPStatus" ->
 * HTTP, Status; "ephemeral_5m" -> ephemeral, 5m.
 */
const KEY_WORD = /\p{Lu}+(?!\p{Ll})|\p{Lu}?[\p{Ll}\p{Lo}\p{Lm}]+|\d+\p{Ll}*/gu;

/**
 * Snake, dot, kebab and camel keys as a sentence: "cache_read_tokens" -> "Cache read tokens",
 * "userID" -> "User ID". Acronyms keep their capitals unless the whole key is in capitals.
 */
export function humanize(key: string): string {
	const words = key.match(KEY_WORD);
	if (!words) return key;
	const shouting = key === key.toUpperCase();
	const sentence = words
		.map((word) =>
			!shouting && word.length > 1 && word === word.toUpperCase() ? word : word.toLowerCase()
		)
		.join(' ');
	return sentence.charAt(0).toUpperCase() + sentence.slice(1);
}
