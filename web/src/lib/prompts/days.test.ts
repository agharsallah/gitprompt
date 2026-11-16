import { describe, expect, it } from 'vitest';
import { byDay } from '#lib/prompts/days.ts';
import { prompt } from '#lib/testing/builders.ts';

// Midday UTC: the same local day in every time zone the tests run in.
const on = (day: string, id: number) => prompt({ id, received_at: `${day}T12:00:00.000Z` });
const shape = (prompts: ReturnType<typeof on>[]) =>
	byDay(prompts).map((d) => [d.key, d.prompts.map((p) => p.id)]);

describe('byDay', () => {
	it('groups the prompts of one day, newest day first', () => {
		expect(shape([on('2026-11-05', 3), on('2026-11-05', 2), on('2026-11-04', 1)])).toEqual([
			['2026-11-05', [3, 2]],
			['2026-11-04', [1]]
		]);
	});

	it('keeps a day whole when its prompts are spread through the list (imported history)', () => {
		// Newest id first, but id 4 was imported later with an older day.
		const list = [
			on('2026-11-05', 5),
			on('2026-11-03', 4),
			on('2026-11-05', 3),
			on('2026-11-04', 2)
		];
		const days = shape(list);
		expect(days).toEqual([
			['2026-11-05', [5, 3]],
			['2026-11-04', [2]],
			['2026-11-03', [4]]
		]);
		expect(new Set(days.map(([key]) => key)).size).toBe(days.length);
	});

	it('puts prompts without a readable date last', () => {
		const undated = prompt({ id: 9, received_at: 'not a date' });
		expect(byDay([undated, on('2026-11-05', 1)]).map((d) => d.prompts[0].id)).toEqual([1, 9]);
	});

	it('gives nothing for no prompts', () => {
		expect(byDay([])).toEqual([]);
	});
});
