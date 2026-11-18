import { describe, expect, it } from 'vitest';
import { tabKeyTarget } from '#lib/ui/tabs.ts';

const three = [{}, {}, {}];

describe('tabKeyTarget', () => {
	it('moves with the arrows and goes round at the ends', () => {
		expect(tabKeyTarget('ArrowRight', 0, three)).toBe(1);
		expect(tabKeyTarget('ArrowRight', 2, three)).toBe(0);
		expect(tabKeyTarget('ArrowLeft', 0, three)).toBe(2);
		expect(tabKeyTarget('ArrowLeft', 2, three)).toBe(1);
	});

	it('jumps to the ends with Home and End', () => {
		expect(tabKeyTarget('Home', 2, three)).toBe(0);
		expect(tabKeyTarget('End', 0, three)).toBe(2);
	});

	it('skips a disabled tab', () => {
		const tabs = [{}, { disabled: true }, {}];
		expect(tabKeyTarget('ArrowRight', 0, tabs)).toBe(2);
		expect(tabKeyTarget('ArrowLeft', 2, tabs)).toBe(0);
	});

	it('leaves other keys alone, and a row with no tab to go to', () => {
		expect(tabKeyTarget('Enter', 0, three)).toBeNull();
		expect(tabKeyTarget('a', 0, three)).toBeNull();
		expect(tabKeyTarget('ArrowRight', 0, [{ disabled: true }])).toBeNull();
	});
});
