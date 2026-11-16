import { describe, expect, it } from 'vitest';
import type { Availability } from '#lib/sources/capabilities.ts';
import { missingNotice } from '#lib/sources/missing.ts';

const of = (a: Partial<Availability>): Availability => ({
	capability: 'cost',
	available: [],
	connectable: [],
	updatable: [],
	unavailable: [],
	...a
});
const hooks = { tool: 'Claude Code', toolId: 'claude-code', layer: 'Hooks' };
const codex = { tool: 'OpenAI Codex CLI', version: '0.164.0' };

describe('missingNotice', () => {
	it('says a tool that reports it has nothing yet, and offers no connect', () => {
		const n = missingNotice('costs', of({ available: ['Claude Code'], unavailable: ['Cursor'] }));
		expect(n.title).toBe('No costs yet');
		expect(n.message).toBe(
			'Claude Code reported no costs for these prompts yet. Not available for Cursor.'
		);
		expect(n.next).toBeUndefined();
	});

	it('names the layer to connect, and links to that tool', () => {
		const n = missingNotice('costs', of({ connectable: [hooks] }));
		expect(n.title).toBe('Costs not available');
		expect(n.message).toBe('Costs come from Claude Code’s hooks.');
		expect(n.next).toEqual({
			label: 'Connect Claude Code',
			href: '/sources#tool-claude-code',
			title: 'Connectors › Claude Code'
		});
	});

	it('says which version would report it', () => {
		const n = missingNotice('models', of({ updatable: [codex], unavailable: ['Cursor'] }));
		expect(n.message).toBe(
			'Models need a newer version: Needs OpenAI Codex CLI 0.164.0 or later. Not available for Cursor.'
		);
	});

	it('says no connector reports it, in the singular and the plural', () => {
		expect(missingNotice('costs', of({ unavailable: ['Cursor'] })).message).toBe(
			'Not available for Cursor: its connector doesn’t report costs.'
		);
		expect(missingNotice('costs', of({ unavailable: ['Cursor', 'Aider'] })).message).toBe(
			'Not available for Cursor and Aider: their connectors don’t report costs.'
		);
		expect(missingNotice('costs', of({})).message).toBe(
			'Not available for the connected tools: its connector doesn’t report costs.'
		);
	});
});
