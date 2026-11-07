import { describe, expect, it } from 'vitest';
import { columnPath, durationTicks, niceTicks, SEQ_STEPS, seqStep } from '#lib/ui/charts/scale.ts';

describe('niceTicks', () => {
	it.each([
		[200, [0, 100, 200]],
		[7, [0, 2.5, 5, 7.5]],
		[13, [0, 5, 10, 15]],
		[0.9, [0, 0.5, 1]],
		[1, [0, 0.5, 1]]
	])('max %s -> %j', (max, ticks) => expect(niceTicks(max)).toEqual(ticks));

	it('always reaches the maximum', () => {
		for (const max of [3, 17, 99, 101, 2345, 0.07])
			expect(niceTicks(max).at(-1)).toBeGreaterThanOrEqual(max);
	});

	it('keeps a baseline for an all-zero series', () => {
		expect(niceTicks(0)).toEqual([0]);
		expect(niceTicks(Number.NaN)).toEqual([0]);
	});
});

describe('durationTicks', () => {
	it('steps in units a reader can count', () => {
		expect(durationTicks(20)).toEqual([0, 10, 20]);
		expect(durationTicks(600)).toEqual([0, 300, 600]);
		expect(durationTicks(1900)).toEqual([0, 900, 1800, 2700]);
	});

	it('keeps a baseline for nothing', () => expect(durationTicks(0)).toEqual([0]));
});

describe('seqStep', () => {
	it('is 0 for nothing and the strongest step for the maximum', () => {
		expect(seqStep(0, 10)).toBe(0);
		expect(seqStep(10, 10)).toBe(SEQ_STEPS);
	});

	it('gives any positive value at least the first step', () => expect(seqStep(0.01, 100)).toBe(1));

	it('grows with the value', () => {
		const steps = [1, 3, 5, 7, 10].map((v) => seqStep(v, 10));
		expect(steps).toEqual([...steps].sort((a, b) => a - b));
	});
});

describe('columnPath', () => {
	it('draws nothing for an empty column', () => expect(columnPath(0, 10, 20, 0)).toBe(''));

	it('rounds the top and keeps the baseline square', () => {
		const d = columnPath(0, 0, 20, 50);
		expect(d.startsWith('M0,50')).toBe(true);
		expect(d).toContain('Q0,0 4,0');
		expect(d.endsWith('V50Z')).toBe(true);
	});

	it('never rounds more than the column allows', () => {
		expect(columnPath(0, 0, 4, 2)).toContain('Q0,0 2,0');
	});
});
