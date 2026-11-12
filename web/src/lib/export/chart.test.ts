import { describe, expect, it } from 'vitest';
import { svgSize } from '#lib/export/chart.ts';

describe('svgSize', () => {
	it('reads the size the root element declares', () => {
		expect(
			svgSize('<svg xmlns="x" width="800" height="412.5"><rect width="5" height="5"/></svg>')
		).toEqual({
			width: 800,
			height: 412.5
		});
	});

	it('ignores sizes of inner elements and lookalike attributes', () => {
		expect(
			svgSize('<svg stroke-width="3" data-height="9"><rect width="5" height="5"/></svg>')
		).toEqual({
			width: 640,
			height: 320
		});
	});

	it('falls back to the default chart size for relative or missing sizes', () => {
		expect(svgSize('<svg width="100%" height="auto"></svg>')).toEqual({ width: 640, height: 320 });
		expect(svgSize('not an svg')).toEqual({ width: 640, height: 320 });
	});
});
