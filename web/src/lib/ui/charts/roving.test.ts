import { describe, expect, it } from 'vitest';
import { rovingStep, tabStop } from '#lib/ui/charts/roving.ts';

describe('rovingStep: keyboard access to a row of columns', () => {
	it.each([
		['ArrowRight', 2, 3],
		['ArrowLeft', 2, 1],
		['Home', 4, 0],
		['End', 0, 6]
	])('%s from column %i moves to column %i', (key, from, to) => {
		expect(rovingStep(key, from, 7)).toEqual({ kind: 'move', index: to });
	});

	it('stops at the ends', () => {
		expect(rovingStep('ArrowRight', 6, 7)).toEqual({ kind: 'move', index: 6 });
		expect(rovingStep('ArrowLeft', 0, 7)).toEqual({ kind: 'move', index: 0 });
	});

	it('goes round with wrap', () => {
		expect(rovingStep('ArrowRight', 6, 7, true)).toEqual({ kind: 'move', index: 0 });
		expect(rovingStep('ArrowLeft', 0, 7, true)).toEqual({ kind: 'move', index: 6 });
	});

	it('starts from the first column (→) or the last (←) when none has focus', () => {
		expect(rovingStep('ArrowRight', null, 7)).toEqual({ kind: 'move', index: 0 });
		expect(rovingStep('ArrowLeft', null, 7)).toEqual({ kind: 'move', index: 6 });
	});

	it('opens the focused column with Enter, and nothing without one', () => {
		expect(rovingStep('Enter', 3, 7)).toEqual({ kind: 'open', index: 3 });
		expect(rovingStep('Enter', null, 7)).toEqual({ kind: 'none' });
	});

	it('lets go with Escape and ignores other keys and empty rows', () => {
		expect(rovingStep('Escape', 3, 7)).toEqual({ kind: 'leave' });
		expect(rovingStep('a', 3, 7)).toEqual({ kind: 'none' });
		expect(rovingStep('ArrowRight', null, 0)).toEqual({ kind: 'none' });
	});
});

describe('tabStop: the one column in the tab order', () => {
	it('is the focused column, else the fallback (today), else the first', () => {
		expect(tabStop(2, 6, 7)).toBe(2);
		expect(tabStop(null, 6, 7)).toBe(6);
		expect(tabStop(null, -1, 7)).toBe(0);
		expect(tabStop(null, 9, 7)).toBe(0);
	});
});
