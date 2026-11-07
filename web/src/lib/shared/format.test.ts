import { describe, expect, it } from 'vitest';
import {
	compact,
	dayKey,
	dayLabel,
	duration,
	excerpt,
	folder,
	humanize,
	modelName,
	money,
	percent,
	plural,
	relative,
	relativePath,
	shortDuration
} from '#lib/shared/format.ts';

describe('money', () => {
	it.each([
		[0, '$0'],
		[undefined, '$0'],
		[0.004, '<$0.01'],
		[0.5879, '$0.59'],
		[12.4, '$12.40'],
		[448.2, '$448'],
		[12_345.6, '$12,346']
	])('%s -> %s', (usd, text) => expect(money(usd)).toBe(text));
});

describe('duration', () => {
	it.each([
		[0, '0s'],
		[2.5, '2.5s'],
		[4, '4s'],
		[42.4, '42s'],
		[59.6, '60s'],
		[65, '1m 05s'],
		[3600, '1h 00m'],
		[4380, '1h 13m'],
		[90_000, '1d 1h']
	])('%s s -> %s', (s, text) => expect(duration(s)).toBe(text));

	it('shows a dash for missing values', () => {
		expect(duration(undefined)).toBe('—');
		expect(duration(Number.NaN)).toBe('—');
	});
});

describe('shortDuration', () => {
	it.each([
		[0, '0'],
		[30, '30s'],
		[300, '5m'],
		[90, '1.5m'],
		[7200, '2h'],
		[172_800, '2d']
	])('%s s -> %s', (s, text) => expect(shortDuration(s)).toBe(text));
});

describe('compact', () => {
	it('keeps numbers under ten thousand in full', () => expect(compact(9876)).toBe('9,876'));
	it('abbreviates large numbers', () => {
		expect(compact(12_900)).toBe('12.9K');
		expect(compact(1_100_000_000)).toBe('1.1B');
	});
});

describe('percent', () => {
	it('rounds to whole percents', () => expect(percent(0.987)).toBe('99%'));
	it('keeps a decimal under 1% so it is not read as zero', () =>
		expect(percent(0.004)).toBe('0.4%'));
	it('shows zero as zero', () => expect(percent(0)).toBe('0%'));
	it('shows a dash without a value', () => expect(percent(null)).toBe('—'));
});

describe('plural', () => {
	it('uses the singular for one', () => expect(plural(1, 'prompt')).toBe('1 prompt'));
	it('adds an s otherwise', () => expect(plural(0, 'prompt')).toBe('0 prompts'));
	it('takes an irregular plural', () => expect(plural(2, 'entry', 'entries')).toBe('2 entries'));
	it('formats the number', () => expect(plural(1200, 'call')).toBe('1,200 calls'));
});

describe('modelName', () => {
	it.each([
		['claude-opus-5-5', 'Opus 5.5'],
		['claude-haiku-5-5', 'Haiku 5.5'],
		['claude-sonnet-4-5-20250929', 'Sonnet 4.5'],
		['claude-opus-4-20250514', 'Opus 4'],
		['claude-opus-5-5[1m]', 'Opus 5.5'],
		['gpt-5', 'GPT-5'],
		['gpt-6-luna', 'GPT-6 Luna'],
		['gpt-5-codex', 'GPT-5 Codex'],
		['gpt-5.1-codex-mini', 'GPT-5.1 Codex Mini'],
		['gpt-4o', 'GPT-4o'],
		['gpt-4o-2024-08-06', 'GPT-4o'],
		['o3', 'o3'],
		['some-local-model', 'some-local-model']
	])('%s -> %s', (id, name) => expect(modelName(id)).toBe(name));
});

describe('relative', () => {
	const now = Date.parse('2026-11-07T12:00:00Z');
	it.each([
		['2026-11-07T11:59:30Z', 'just now'],
		['2026-11-07T11:55:00Z', '5 minutes ago'],
		['2026-11-07T09:00:00Z', '3 hours ago'],
		['2026-11-06T12:00:00Z', 'yesterday']
	])('%s -> %s', (iso, text) => expect(relative(iso, now)).toBe(text));

	it('says never without a time', () => expect(relative(null, now)).toBe('never'));
});

describe('dayKey and dayLabel', () => {
	it('groups by the local calendar day', () => {
		// 23:30 UTC on the 5th is already the 6th in Berlin (UTC+1 in November).
		expect(dayKey('2026-11-05T23:30:00Z')).toBe('2026-11-06');
	});

	it('names today and yesterday, and spells out other days', () => {
		const now = new Date(2026, 10, 7, 12);
		expect(dayLabel('2026-11-07', now)).toBe('Today');
		expect(dayLabel('2026-11-06', now)).toBe('Yesterday');
		expect(dayLabel('2026-11-05', now)).toBe('Thursday 5 November');
		// ICU adds a comma after the weekday when the year is shown; either way is the locale's call.
		expect(dayLabel('2025-11-05', now)).toMatch(/^Wednesday,? 5 November 2025$/);
	});
});

describe('excerpt', () => {
	it('keeps short text as one line', () => expect(excerpt('a\n  b')).toBe('a b'));

	it('cuts long text on a word boundary', () => {
		const text = 'word '.repeat(40);
		const cut = excerpt(text, 30);
		expect(cut.endsWith('…')).toBe(true);
		expect(cut.length).toBeLessThanOrEqual(31);
		expect(cut).not.toMatch(/wor…$/);
	});

	it('is empty for no text', () => expect(excerpt(null)).toBe(''));
});

describe('paths and keys', () => {
	it('takes the last folder', () => {
		expect(folder('/work/app/')).toBe('app');
		expect(folder(undefined)).toBeNull();
	});

	it('shows paths relative to the repository, or to home', () => {
		expect(relativePath('/work/app/src/a.ts', '/work/app')).toBe('src/a.ts');
		expect(relativePath('/Users/me/.claude/settings.json')).toBe('~/.claude/settings.json');
	});

	it('turns field names into words', () => {
		expect(humanize('cache_read_tokens')).toBe('Cache read tokens');
		expect(humanize('active_time.total[user]')).toBe('Active time total user');
	});
});
