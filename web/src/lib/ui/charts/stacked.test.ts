import { describe, expect, it } from 'vitest';
import { stackSegments } from '#lib/ui/charts/stacked.ts';

describe('stackSegments', () => {
	it('stacks the parts bottom-up in their order, skipping empty ones; the last is the top', () => {
		expect(
			stackSegments([
				{ series: 'a', value: 2 },
				{ series: 'b', value: 0 },
				{ series: 'c', value: 3 }
			])
		).toEqual([
			{ series: 'a', from: 0, to: 2, top: false },
			{ series: 'c', from: 2, to: 5, top: true }
		]);
	});

	it('draws nothing for a quiet column', () => {
		expect(stackSegments([{ series: 'a', value: 0 }])).toEqual([]);
	});
});
