import { describe, expect, it } from 'vitest';
import type { UsageOut } from '#lib/api/types.ts';
import { shiftDay, usageRows } from '#lib/settings/usage.ts';
import { USAGE_ACTIONS } from '#lib/shared/usage-actions.ts';

const out = (counts: UsageOut['counts'], until = '2026-01-30'): UsageOut => ({
	since: '2026-01-01',
	until,
	counts
});

describe('shiftDay', () => {
	it('moves a day on the calendar across months and years', () => {
		expect(shiftDay('2026-01-30', -6)).toBe('2026-01-24');
		expect(shiftDay('2026-01-03', -6)).toBe('2025-12-28');
		expect(shiftDay('2026-03-01', -1)).toBe('2026-02-28');
	});
});

describe('usageRows', () => {
	it('sums the last 7 days (until included) and the whole range per action', () => {
		const rows = usageRows(
			out([
				{ action: 'copy.prompt', day: '2026-01-10', n: 5 },
				{ action: 'copy.prompt', day: '2026-01-24', n: 2 },
				{ action: 'copy.prompt', day: '2026-01-30', n: 1 },
				{ action: 'copy.prompt', day: '2026-01-23', n: 4 }
			])
		);
		expect(rows.find((r) => r.action === 'copy.prompt')).toEqual({
			action: 'copy.prompt',
			week: 3,
			month: 12
		});
	});

	it('lists every known action, unused ones at 0, most used first', () => {
		const rows = usageRows(
			out([
				{ action: 'export.open', day: '2026-01-30', n: 1 },
				{ action: 'reader.tab.raw', day: '2026-01-30', n: 3 }
			])
		);
		expect(rows).toHaveLength(USAGE_ACTIONS.length);
		expect(rows.slice(0, 2).map((r) => r.action)).toEqual(['reader.tab.raw', 'export.open']);
		expect(rows.at(-1)).toMatchObject({ week: 0, month: 0 });
	});

	it('keeps an action the dashboard no longer knows', () => {
		const rows = usageRows(out([{ action: 'old.thing', day: '2026-01-30', n: 2 }]));
		expect(rows[0]).toEqual({ action: 'old.thing', week: 2, month: 2 });
	});
});
