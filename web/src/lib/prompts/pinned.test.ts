import { describe, expect, it } from 'vitest';
import { pins, unpin } from '#lib/prompts/pinned.ts';

const none = { model: [], outcome: [], has: [] };

describe('pins', () => {
	it('labels each doorway filter', () => {
		const labels = pins(
			{
				repo: '/r/app',
				weekday: '1',
				hour: '9',
				tool_kind: 'shell',
				size: '00020',
				command: '/review'
			},
			{ model: ['claude-opus-5-5'], outcome: ['superseded'], has: ['commit'] }
		).map((p) => p.label);
		expect(labels).toEqual([
			'app',
			'Mondays',
			'09:00 to 10:00',
			'/review',
			'Shell',
			'20 to 80 characters',
			'Opus 5.5',
			'Superseded',
			'Agent committed'
		]);
	});

	it('shows nothing for empty filters, and keeps hour 0', () => {
		expect(pins({}, none)).toEqual([]);
		expect(pins({ hour: '0' }, none)[0].label).toBe('00:00 to 01:00');
	});
});

describe('unpin', () => {
	it('removes one value of a list and the selection, and keeps the rest', () => {
		expect(unpin('has=code&has=url&id=4&repo=x', { key: 'has', value: 'code', label: '' })).toBe(
			'repo=x&has=url'
		);
	});
});
