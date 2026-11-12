import { describe, expect, it } from 'vitest';
import { pinLabel, pins, unpin } from '#lib/prompts/pinned.ts';

const none = { model: [], surface: [], outcome: [], has: [] };

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
			{ model: ['claude-opus-5-5'], surface: [], outcome: ['superseded'], has: ['commit'] }
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

describe('labels of the other doorways', () => {
	it('names a session, a source, a tool, model calls and older pages', () => {
		const labels = pins(
			{
				session: '0123456789abcdef',
				source: 'codex',
				tool: 'Bash',
				query_source: 'compact',
				before: '42'
			},
			none
		).map((p) => p.label);
		expect(labels[0]).toBe('Session 01234567');
		expect(labels).toContain('Source codex');
		expect(labels).toContain('Tool Bash');
		expect(labels.some((l) => l.startsWith('Model calls: '))).toBe(true);
		expect(labels.at(-1)).toBe('Older prompts');
	});

	it('shows an instant range and names the app a prompt was typed in', () => {
		const [from, to] = pins({ since: '2026-03-01T09:30:00Z', until: '2026-03-02T09:30:00Z' }, none);
		expect(from.label.startsWith('From ')).toBe(true);
		expect(to.label.startsWith('Before ')).toBe(true);
		expect(pins({}, { ...none, surface: ['vscode'] }).map((p) => p.label)).toEqual([
			'Typed in vscode'
		]);
	});

	it('falls back for a weekday or size it does not know', () => {
		expect(pins({ weekday: '9' }, none)[0].label).toBe('Weekday 9');
		expect(pins({ size: 'huge' }, none)[0].label).toBe('huge characters');
	});
});

describe('pinLabel', () => {
	it('labels one pinned value, list or single', () => {
		expect(pinLabel('has', 'commit')).toBe('Agent committed');
		expect(pinLabel('hour', '23')).toBe('23:00 to 00:00');
	});

	it('knows nothing of other keys, including the names every object has', () => {
		for (const key of ['q', 'toString', 'constructor', 'hasOwnProperty', '__proto__'])
			expect(pinLabel(key, 'x')).toBeUndefined();
	});
});

describe('unpin', () => {
	it('removes one value of a list and the selection, and keeps the rest', () => {
		expect(unpin('has=code&has=url&id=4&repo=x', { key: 'has', value: 'code', label: '' })).toBe(
			'repo=x&has=url'
		);
	});
});
