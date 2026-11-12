import { describe, expect, it } from 'vitest';
import {
	compact,
	count,
	dayKey,
	dayLabel,
	duration,
	excerpt,
	folder,
	humanize,
	inline,
	integer,
	modelName,
	money,
	percent,
	pick,
	plural,
	present,
	relative,
	relativePath,
	shortDate,
	shortDuration,
	timeOfDay,
	when
} from '#lib/shared/format.ts';

// Model naming has its own suite (model-name.test.ts); format.ts re-exports it.
it('re-exports modelName', () => expect(modelName('claude-opus-5-5')).toBe('Opus 5.5'));

describe('pick', () => {
	const data = { a: 1, b: 2, c: 3 };
	it('keeps the listed keys', () => expect(pick(data, ['a', 'c'])).toEqual({ a: 1, c: 3 }));
	it('keeps the other keys with except', () =>
		expect(pick(data, ['a'], true)).toEqual({ b: 2, c: 3 }));
	it('ignores keys that are not there', () => expect(pick(data, ['z'])).toEqual({}));
});

describe('folder', () => {
	it.each([
		['/work/app', 'app'],
		['/work/app/', 'app'],
		['/work/app//', 'app'],
		['app', 'app'],
		['C:\\work\\app', 'app'],
		['C:\\work\\app\\', 'app']
	])('%s -> %s', (path, name) => expect(folder(path)).toBe(name));

	it.each([['/'], [''], ['//'], [undefined], [null], [42]])('%j has no folder', (path) =>
		expect(folder(path)).toBeNull()
	);
});

describe('count', () => {
	it('counts a list', () => expect(count([1, 2, 3])).toBe(3));
	it('sums a map', () => expect(count({ Read: 3, Edit: 2 })).toBe(5));
	it('reads numbers written as strings', () => expect(count({ a: '3', b: 2 })).toBe(5));
	it('counts values that are not numbers as zero', () =>
		expect(count({ a: 1, b: 'x', c: null, d: '', e: Number.NaN, f: {} })).toBe(1));
	it.each([[undefined], [null], [7], ['abc'], [true]])('is 0 for %j', (value) =>
		expect(count(value)).toBe(0)
	);
});

describe('present', () => {
	it('drops the nulls the daemon pads a list with', () =>
		expect(present(['a', null, 'b'])).toEqual(['a', 'b']));
	it('reads a missing list as empty', () => expect(present(undefined)).toEqual([]));
	it('keeps falsy values that are there', () => expect(present([0, '', null])).toEqual([0, '']));
});

describe('inline', () => {
	it('keeps a string as is', () => expect(inline('a "b"')).toBe('a "b"'));
	it('writes other values as JSON', () => {
		expect(inline({ a: [1, true] })).toBe('{"a":[1,true]}');
		expect(inline(null)).toBe('null');
		expect(inline(3)).toBe('3');
	});
	it('always returns a string', () => expect(inline(undefined)).toBe('undefined'));
});

describe('compact', () => {
	it.each([
		[0, '0'],
		[9876, '9,876'],
		[9999.4, '9,999'],
		[-9876, '-9,876'],
		[-0.4, '0'],
		[12_900, '12.9K'],
		[1_100_000_000, '1.1B'],
		[-12_900, '-12.9K']
	])('%s -> %s', (n, text) => expect(compact(n)).toBe(text));

	it('shows a dash for a non-number', () => {
		expect(compact(Number.NaN)).toBe('—');
		expect(compact(Infinity)).toBe('—');
	});
});

describe('integer', () => {
	it.each([
		[1234.6, '1,235'],
		[0, '0'],
		[-0.4, '0'],
		[-1500, '-1,500']
	])('%s -> %s', (n, text) => expect(integer(n)).toBe(text));
	it('shows a dash for a non-number', () => expect(integer(Number.NaN)).toBe('—'));
});

describe('money', () => {
	it.each([
		[0, '$0'],
		[-0, '$0'],
		[undefined, '$0'],
		[null, '$0'],
		[0.004, '<$0.01'],
		[0.01, '$0.01'],
		[0.5879, '$0.59'],
		[12.4, '$12.40'],
		[99.99, '$99.99'],
		// Rounds to $100.00, so it is shown in whole dollars.
		[99.996, '$100'],
		[100, '$100'],
		[448.2, '$448'],
		[12_345.6, '$12,346'],
		[-0.004, '>-$0.01'],
		[-5, '-$5.00'],
		[-1234, '-$1,234']
	])('%s -> %s', (usd, text) => expect(money(usd)).toBe(text));

	it('shows a dash for a non-number', () => {
		expect(money(Number.NaN)).toBe('—');
		expect(money(Infinity)).toBe('—');
	});
});

describe('percent', () => {
	it.each([
		[0.987, '99%'],
		[1, '100%'],
		[12.34, '1,234%'],
		[0, '0%'],
		[0.004, '0.4%'],
		[0.0094, '0.9%'],
		// 0.96% rounds to 1.0%: shown whole, not as "1.0%".
		[0.0096, '1%'],
		// So small it would round to 0.0%.
		[0.0000004, '<0.1%'],
		[-0.003, '0%'],
		[-0.25, '-25%']
	])('%s -> %s', (ratio, text) => expect(percent(ratio)).toBe(text));

	it('shows a dash without a value', () => {
		expect(percent(null)).toBe('—');
		expect(percent(undefined)).toBe('—');
		expect(percent(Number.NaN)).toBe('—');
	});
});

describe('plural', () => {
	it('uses the singular for one', () => expect(plural(1, 'prompt')).toBe('1 prompt'));
	it('adds an s otherwise', () => expect(plural(0, 'prompt')).toBe('0 prompts'));
	it('takes an irregular plural', () => expect(plural(2, 'entry', 'entries')).toBe('2 entries'));
	it('formats the number', () => expect(plural(1200, 'call')).toBe('1,200 calls'));
	it('agrees with the number shown, not the number given', () => {
		expect(plural(0.6, 'prompt')).toBe('1 prompt');
		expect(plural(1.4, 'prompt')).toBe('1 prompt');
		expect(plural(1.6, 'prompt')).toBe('2 prompts');
	});
});

describe('duration', () => {
	it.each([
		[0, '0s'],
		[-5, '0s'],
		[0.04, '0s'],
		[2.5, '2.5s'],
		[4, '4s'],
		[9.96, '10s'],
		[42.4, '42s'],
		// Rounded before it is split into units: never "60s" or "59m 60s".
		[59.6, '1m 00s'],
		[65, '1m 05s'],
		[3599.6, '1h 00m'],
		[3600, '1h 00m'],
		[4380, '1h 13m'],
		[86_399.6, '1d 0h'],
		[90_000, '1d 1h'],
		[10 * 86_400 + 5 * 3600, '10d 5h']
	])('%s s -> %s', (s, text) => expect(duration(s)).toBe(text));

	it('shows a dash for missing values', () => {
		expect(duration(undefined)).toBe('—');
		expect(duration(null)).toBe('—');
		expect(duration(Number.NaN)).toBe('—');
		expect(duration(Infinity)).toBe('—');
	});
});

describe('shortDuration', () => {
	it.each([
		[0, '0'],
		[0.5, '0.5s'],
		[0.01, '0s'],
		[30, '30s'],
		[300, '5m'],
		[90, '1.5m'],
		[59.99, '1m'],
		[3599, '1h'],
		[7200, '2h'],
		[172_800, '2d'],
		[1000 * 86_400, '1,000d'],
		[-300, '-5m']
	])('%s s -> %s', (s, text) => expect(shortDuration(s)).toBe(text));

	it('shows a dash for a non-number', () => expect(shortDuration(Number.NaN)).toBe('—'));
});

// The suite runs in Europe/Berlin (vite.config.ts), so local times are known.
describe('when', () => {
	it('shows a date and time', () => expect(when('2026-11-07T12:00:00Z')).toMatch(/2026/));
	it.each([[null], [undefined], [''], ['not a date']])('shows a dash for %j', (iso) =>
		expect(when(iso)).toBe('—')
	);
});

describe('relative', () => {
	const now = Date.parse('2026-11-07T12:00:00Z');
	it.each([
		['2026-11-07T11:59:30Z', 'just now'],
		['2026-11-07T12:00:30Z', 'just now'],
		['2026-11-07T11:59:00Z', '1 minute ago'],
		['2026-11-07T11:55:00Z', '5 minutes ago'],
		// 59.6 minutes rounds up to the next unit, not to "60 minutes ago".
		['2026-11-07T11:00:24Z', '1 hour ago'],
		['2026-11-07T09:00:00Z', '3 hours ago'],
		['2026-11-06T12:20:00Z', 'yesterday'],
		['2026-11-06T12:00:00Z', 'yesterday'],
		['2026-11-04T12:00:00Z', '3 days ago'],
		['2026-11-07T15:00:00Z', 'in 3 hours'],
		['2026-11-08T12:00:00Z', 'tomorrow'],
		// From a week away it is a date, in the year of `now`.
		['2026-10-31T13:00:00Z', '31 Oct'],
		['2025-11-05T12:00:00Z', '5 Nov 2025']
	])('%s -> %s', (iso, text) => expect(relative(iso, now)).toBe(text));

	it('says never without a time', () => {
		expect(relative(null, now)).toBe('never');
		expect(relative('', now)).toBe('never');
	});
	it('shows a dash for a time that does not parse', () => expect(relative('soon', now)).toBe('—'));
});

describe('shortDate', () => {
	const now = new Date(2026, 10, 7);
	it('leaves out the current year', () =>
		expect(shortDate('2026-11-05T12:00:00Z', now)).toBe('5 Nov'));
	it('shows another year', () => expect(shortDate('2025-11-05T12:00:00Z', now)).toBe('5 Nov 2025'));
	it('takes a Date', () => expect(shortDate(new Date(2026, 0, 2), now)).toBe('2 Jan'));
	it('shows a dash for a date that does not parse', () => expect(shortDate('nope', now)).toBe('—'));
});

describe('timeOfDay', () => {
	// 24- or 12-hour, as the viewer's locale has it.
	it('shows hours and minutes', () =>
		expect(timeOfDay('2026-11-07T13:05:00Z')).toMatch(/^(14:05|0?2:05\s?PM)$/i));
	it('shows a dash for a time that does not parse', () => expect(timeOfDay('nope')).toBe('—'));
});

describe('dayKey and dayLabel', () => {
	it('groups by the local calendar day', () => {
		// 23:30 UTC on the 5th is already the 6th in Berlin (UTC+1 in November).
		expect(dayKey('2026-11-05T23:30:00Z')).toBe('2026-11-06');
		expect(dayKey(new Date(2026, 0, 2, 3))).toBe('2026-01-02');
	});

	it('keys a date that does not parse as a dash', () => {
		expect(dayKey('nope')).toBe('—');
		expect(dayKey(new Date(Number.NaN))).toBe('—');
	});

	it('names today and yesterday, and spells out other days', () => {
		const now = new Date(2026, 10, 7, 12);
		expect(dayLabel('2026-11-07', now)).toBe('Today');
		expect(dayLabel('2026-11-06', now)).toBe('Yesterday');
		expect(dayLabel('2026-11-05', now)).toBe('Thursday 5 November');
		// ICU adds a comma after the weekday when the year is shown; either way is the locale's call.
		expect(dayLabel('2025-11-05', now)).toMatch(/^Wednesday,? 5 November 2025$/);
	});

	it('finds yesterday across a month and a daylight-saving change', () => {
		// Clocks went back in Berlin on 25 October 2026.
		expect(dayLabel('2026-10-25', new Date(2026, 9, 26, 0, 30))).toBe('Yesterday');
		expect(dayLabel('2026-10-31', new Date(2026, 10, 1, 0, 30))).toBe('Yesterday');
	});

	it.each([['2026-02-30'], ['2026-13-01'], ['—'], [''], ['yesterday'], ['2026-11']])(
		'gives back %j, which is not a day key',
		(key) => expect(dayLabel(key, new Date(2026, 10, 7))).toBe(key)
	);
});

describe('excerpt', () => {
	it('keeps short text as one line', () => expect(excerpt('  a\n\t b  ')).toBe('a b'));
	it('keeps text of exactly max characters', () => expect(excerpt('abcde', 5)).toBe('abcde'));

	it('cuts long text on a word boundary', () => {
		const text = 'word '.repeat(40);
		const cut = excerpt(text, 30);
		expect(cut.endsWith('…')).toBe(true);
		expect(cut.length).toBeLessThanOrEqual(31);
		expect(cut).not.toMatch(/wor…$/);
	});

	it('cuts mid-word when the last space is too early', () =>
		expect(excerpt('a verylongwordthatkeepsgoing', 12)).toBe('a verylongwo…'));

	it('does not split an emoji', () => {
		const cut = excerpt('x'.repeat(9) + '😀😀', 10);
		expect(cut).toBe(`${'x'.repeat(9)}…`);
		expect(cut.isWellFormed()).toBe(true);
	});

	it('is empty for no text', () => {
		expect(excerpt(null)).toBe('');
		expect(excerpt(undefined)).toBe('');
		expect(excerpt('')).toBe('');
	});
});

describe('relativePath', () => {
	it.each([
		['/work/app/src/a.ts', '/work/app', 'src/a.ts'],
		['/work/app/src/a.ts', '/work/app/', 'src/a.ts'],
		// A sibling that shares the root's prefix is not under it.
		['/work/application/a.ts', '/work/app', '/work/application/a.ts'],
		['/Users/me/app/a.ts', '/Users/me/app', 'a.ts']
	])('%s under %s -> %s', (path, root, shown) => expect(relativePath(path, root)).toBe(shown));

	it.each([
		['/Users/me/.claude/settings.json', '~/.claude/settings.json'],
		['/home/me/.config/x', '~/.config/x'],
		['/Users/me', '~'],
		['/Users/me/', '~/'],
		// Not anyone's home.
		['/Users/Shared/data', '/Users/Shared/data'],
		['/Users/', '/Users/'],
		['/opt/app', '/opt/app'],
		['relative/path', 'relative/path']
	])('%s -> %s', (path, shown) => expect(relativePath(path)).toBe(shown));

	it('falls back to home outside the root', () =>
		expect(relativePath('/Users/me/other/a.ts', '/work/app')).toBe('~/other/a.ts'));
	it('ignores an empty or bare-slash root', () => {
		expect(relativePath('/opt/a', '')).toBe('/opt/a');
		expect(relativePath('/opt/a', '/')).toBe('/opt/a');
		expect(relativePath('/opt/a', null)).toBe('/opt/a');
	});
});

describe('humanize', () => {
	it.each([
		['cache_read_tokens', 'Cache read tokens'],
		['active_time.total[user]', 'Active time total user'],
		['cacheReadTokens', 'Cache read tokens'],
		['kebab-case-key', 'Kebab case key'],
		['userID', 'User ID'],
		['HTTPStatus', 'HTTP status'],
		['tool_calls[0].name', 'Tool calls 0 name'],
		['cache_creation.ephemeral_5m_input_tokens', 'Cache creation ephemeral 5m input tokens'],
		['MAX_TOKENS', 'Max tokens'],
		['__private__', 'Private'],
		['x', 'X'],
		['café_au_lait', 'Café au lait']
	])('%s -> %s', (key, text) => expect(humanize(key)).toBe(text));

	it('gives back a key with no words', () => {
		expect(humanize('')).toBe('');
		expect(humanize('_.')).toBe('_.');
	});
});
