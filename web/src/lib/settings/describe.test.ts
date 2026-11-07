import { describe, expect, it } from 'vitest';
import type { CleanupReport, Setting } from '#lib/api/settings.ts';
import { bytes, daysAgo, origin, startOfDay, summary } from '#lib/settings/describe.ts';
import { DEFAULTS, values } from '#lib/settings/preferences.ts';

const setting = (over: Partial<Setting> = {}): Setting => ({
	value: '7d',
	default: '7d',
	source: 'default',
	env: 'GITPROMPT_DEFAULT_RANGE',
	locked: false,
	applies: 'now',
	...over
});

const report = (over: Partial<CleanupReport> = {}): CleanupReport => ({
	removals: [],
	rows: 0,
	bytes: 0,
	size_before: 50_000_000,
	size_after: null,
	applied: false,
	...over
});

describe('where a setting comes from', () => {
	it('names the env var that locks it and how to unlock it', () => {
		const o = origin(setting({ source: 'env', locked: true }));
		expect(o.label).toBe('Set by GITPROMPT_DEFAULT_RANGE');
		expect(o.detail).toContain('Unset GITPROMPT_DEFAULT_RANGE');
	});

	it('tells a saved value from the default', () => {
		expect(origin(setting({ source: 'file' }), '/x/preferences.json').detail).toBe(
			'Saved in /x/preferences.json.'
		);
		expect(origin(setting()).label).toBe('Default');
	});
});

describe('clean-up summaries', () => {
	it('says what a dry run would delete, and that prompts are kept', () => {
		expect(summary(report())).toBe('Nothing to delete.');
		expect(summary(report({ rows: 1200, bytes: 3_400_000 }))).toBe(
			'Would delete 1,200 events (about 3.4 MB). Prompts and notes are kept.'
		);
	});

	it('reports the size change once applied', () => {
		expect(summary(report({ rows: 1, applied: true, size_after: 20_000_000 }))).toBe(
			'Deleted 1 event. The database went from 50.0 MB to 20.0 MB.'
		);
	});

	it('formats sizes', () => {
		expect(bytes(512)).toBe('512 B');
		expect(bytes(4_096)).toBe('4 kB');
		expect(bytes(2_500_000_000)).toBe('2.50 GB');
	});
});

describe('dates', () => {
	it('turns a local calendar day into the ISO time it starts', () => {
		expect(startOfDay('2026-10-01')).toBe(new Date(2026, 9, 1).toISOString());
		expect(startOfDay('')).toBeNull();
		expect(startOfDay('01/10/2026')).toBeNull();
	});

	it('computes the retention cutoff', () => {
		expect(daysAgo(2, Date.parse('2026-11-07T12:00:00Z'))).toBe('2026-11-05T12:00:00.000Z');
	});
});

describe('preference values', () => {
	it('reads the effective value of each setting', () => {
		const state = {
			file: 'preferences.json',
			settings: {
				default_range: setting({ value: '30d', source: 'file' }),
				week_start: setting({ value: 'sunday', default: 'monday' }),
				event_retention_days: setting({ value: null, default: null }),
				hide_secrets_on_ingest: setting({ value: true, default: false })
			}
		} as Parameters<typeof values>[0];
		expect(values(state)).toEqual({
			...DEFAULTS,
			default_range: '30d',
			week_start: 'sunday',
			hide_secrets_on_ingest: true
		});
	});
});
