// Display helpers shared by components. Pure functions, unit tested in format.test.ts.
import type { Meta } from '#lib/api/types.ts';

export const when = (iso: string | null | undefined) =>
	iso ? new Date(iso).toLocaleString() : '—';

/** Keep only the keys of `data` listed in `keys` (or, with `except`, all other keys). */
export const pick = (data: Meta, keys: readonly string[], except = false) =>
	Object.fromEntries(Object.entries(data).filter(([k]) => keys.includes(k) !== except));

/** Last segment of a path. */
export const folder = (path: unknown) =>
	typeof path === 'string' ? (path.replace(/\/+$/, '').split('/').at(-1) ?? null) : null;

/** Items in a list, or the sum of a `{name: count}` map. */
export const count = (value: unknown) =>
	Array.isArray(value)
		? value.length
		: Object.values(value ?? {}).reduce((a: number, b) => a + Number(b), 0);

/** One-line rendering of any JSON value. */
export const inline = (value: unknown) =>
	typeof value === 'string' ? value : JSON.stringify(value);

const compactFormat = new Intl.NumberFormat('en', { notation: 'compact', maximumFractionDigits: 1 });
const integerFormat = new Intl.NumberFormat('en');

/** 1,284 under ten thousand, 12.9K / 4.2M above. */
export const compact = (n: number) =>
	Math.abs(n) < 10_000 ? integerFormat.format(Math.round(n)) : compactFormat.format(n);

/** Full integer with separators, for tables and tooltips. */
export const integer = (n: number) => integerFormat.format(Math.round(n));

/** Dollars: cents under $100, whole dollars above, "<$0.01" for crumbs, "$0" for nothing. */
export function money(usd: number | null | undefined): string {
	if (!usd) return '$0';
	if (usd < 0.01) return '<$0.01';
	if (usd < 100) return `$${usd.toFixed(2)}`;
	return `$${integerFormat.format(Math.round(usd))}`;
}

/** Seconds as 4.2s, 42s, 3m 05s, 1h 12m. */
export function duration(seconds: number | null | undefined): string {
	if (seconds == null || !Number.isFinite(seconds)) return '—';
	const s = Math.max(0, seconds);
	if (s < 10) return `${s.toFixed(1).replace(/\.0$/, '')}s`;
	if (s < 60) return `${Math.round(s)}s`;
	const m = Math.floor(s / 60);
	if (m < 60) return `${m}m ${String(Math.round(s % 60)).padStart(2, '0')}s`;
	const h = Math.floor(m / 60);
	if (h < 24) return `${h}h ${String(m % 60).padStart(2, '0')}m`;
	return `${Math.floor(h / 24)}d ${h % 24}h`;
}

/** A duration for an axis tick: the largest whole unit only (30s, 5m, 2h, 1d). */
export function shortDuration(seconds: number): string {
	if (seconds === 0) return '0';
	if (seconds < 60) return `${seconds}s`;
	if (seconds < 3600) return `${+(seconds / 60).toFixed(1)}m`;
	if (seconds < 86_400) return `${+(seconds / 3600).toFixed(1)}h`;
	return `${+(seconds / 86_400).toFixed(1)}d`;
}

/** A ratio (0..1) as a whole percentage; under 1% shows one decimal so it isn't read as zero. */
export function percent(ratio: number | null | undefined): string {
	if (ratio == null || !Number.isFinite(ratio)) return '—';
	const p = ratio * 100;
	if (p > 0 && p < 1) return `${p.toFixed(1)}%`;
	return `${Math.round(p)}%`;
}

/** "1 prompt", "3 prompts"; irregular plurals pass `many`. */
export const plural = (n: number, one: string, many = `${one}s`) =>
	`${integer(n)} ${n === 1 ? one : many}`;

const relativeFormat = new Intl.RelativeTimeFormat('en', { numeric: 'auto', style: 'long' });

/** "just now", "5 minutes ago", "yesterday", then a short date. */
export function relative(iso: string | null | undefined, now = Date.now()): string {
	if (!iso) return 'never';
	const t = Date.parse(iso);
	if (Number.isNaN(t)) return '—';
	const s = Math.round((t - now) / 1000);
	const abs = Math.abs(s);
	if (abs < 45) return 'just now';
	if (abs < 3600) return relativeFormat.format(Math.round(s / 60), 'minute');
	if (abs < 86_400) return relativeFormat.format(Math.round(s / 3600), 'hour');
	if (abs < 7 * 86_400) return relativeFormat.format(Math.round(s / 86_400), 'day');
	return shortDate(iso);
}

/** "5 Nov", or "5 Nov 2025" outside the current year. */
export function shortDate(iso: string, now = new Date()): string {
	const d = new Date(iso);
	return d.toLocaleDateString('en-GB', {
		day: 'numeric',
		month: 'short',
		year: d.getFullYear() === now.getFullYear() ? undefined : 'numeric'
	});
}

/** "14:05" in the viewer's locale. */
export const timeOfDay = (iso: string) =>
	new Date(iso).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });

/** Local calendar day as YYYY-MM-DD: the key for grouping by day. */
export function dayKey(date: Date | string): string {
	const d = typeof date === 'string' ? new Date(date) : date;
	const pad = (n: number) => String(n).padStart(2, '0');
	return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/** "Today", "Yesterday", or "Thursday 5 November" for a YYYY-MM-DD key. */
export function dayLabel(key: string, now = new Date()): string {
	if (key === dayKey(now)) return 'Today';
	const yesterday = new Date(now);
	yesterday.setDate(now.getDate() - 1);
	if (key === dayKey(yesterday)) return 'Yesterday';
	const [y, m, d] = key.split('-').map(Number);
	const date = new Date(y, m - 1, d);
	return date.toLocaleDateString('en-GB', {
		weekday: 'long',
		day: 'numeric',
		month: 'long',
		year: y === now.getFullYear() ? undefined : 'numeric'
	});
}

/**
 * A model id as people say it: "claude-opus-5-5" -> "Opus 5.5",
 * "claude-sonnet-4-5-20250929" -> "Sonnet 4.5". Unknown ids pass through.
 */
export function modelName(id: string): string {
	const m = id.match(/^claude-([a-z]+)-(\d+)(?:-(\d{1,2}))?(?:-\d{8})?(?:\[.*\])?$/i);
	if (!m) return id.replace(/^claude-/, '');
	const family = m[1][0].toUpperCase() + m[1].slice(1);
	return m[3] ? `${family} ${m[2]}.${m[3]}` : `${family} ${m[2]}`;
}

/**
 * A prompt without the context blocks tools inject into it (an IDE selection, a system reminder),
 * so an excerpt shows what you actually typed. Only used for previews; the full text is kept.
 */
export const withoutContext = (text: string) =>
	text.replace(/<(ide_[a-z_]+|system-reminder|local-command-[a-z]+)>[\s\S]*?<\/\1>/g, '').trim() || text;

/** First line of a text, trimmed to `max` characters on a word boundary. */
export function excerpt(text: string | null | undefined, max = 140): string {
	if (!text) return '';
	const line = withoutContext(text).trim().replace(/\s+/g, ' ');
	if (line.length <= max) return line;
	const cut = line.slice(0, max);
	const space = cut.lastIndexOf(' ');
	return `${(space > max * 0.6 ? cut.slice(0, space) : cut).trimEnd()}…`;
}

/** A path shown relative to a root it lives under (the repository), else as given. */
export function relativePath(path: string, root?: string | null): string {
	if (root && path.startsWith(`${root}/`)) return path.slice(root.length + 1);
	return path.replace(/^\/Users\/[^/]+/, '~').replace(/^\/home\/[^/]+/, '~');
}

/** Snake/dot/camel keys as words: "cache_read_tokens" -> "Cache read tokens". */
export function humanize(key: string): string {
	const words = key
		.replace(/\[(.+?)\]/g, ' $1')
		.replace(/([a-z])([A-Z])/g, '$1 $2')
		.replace(/[._]+/g, ' ')
		.trim()
		.toLowerCase();
	return words.charAt(0).toUpperCase() + words.slice(1);
}
