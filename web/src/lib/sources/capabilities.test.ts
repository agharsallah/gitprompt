import { describe, expect, it } from 'vitest';
import {
	assistantOf,
	availability,
	names,
	note,
	toolCapabilities,
	toolOf
} from '#lib/sources/capabilities.ts';
import { tools } from '#lib/sources/state.ts';
import { source } from '#lib/testing/builders.ts';

const hooks = (o = {}) => source({ capabilities: ['prompt', 'tools', 'turn_end'], ...o });
const telemetry = (o = {}) =>
	source({
		id: 'claude-code-otel',
		part_of: 'claude-code',
		layer: 'Telemetry',
		capabilities: ['model', 'usage', 'cost'],
		...o
	});
const copilot = (o = {}) =>
	source({
		id: 'copilot-cli',
		name: 'GitHub Copilot CLI',
		capabilities: ['prompt', 'tools', 'turn_end'],
		join: 'session',
		...o
	});

describe('toolCapabilities', () => {
	it('unions the active layers and lists what the others would add', () => {
		const [tool] = tools([hooks({ connected: true }), telemetry()]);
		expect(toolCapabilities(tool)).toEqual({
			has: ['prompt', 'turn_end', 'tools'],
			offered: ['model', 'usage', 'cost']
		});
	});

	it('counts a layer that sent data even if disconnected since, but never a turned-off one', () => {
		const [tool] = tools([hooks(), telemetry({ events: 5 })]);
		expect(toolCapabilities(tool).has).toEqual(['model', 'usage', 'cost']);
		const [off] = tools([hooks(), telemetry({ events: 5, enabled: false })]);
		expect(toolCapabilities(off).has).toEqual([]);
	});
});

describe('availability', () => {
	it('names the tools in use that provide, could provide, or never provide a capability', () => {
		const sources = [
			hooks({ connected: true }),
			telemetry({ connected: true }),
			copilot({ connected: true })
		];
		expect(availability(sources, 'usage')).toEqual({
			capability: 'usage',
			available: ['Claude Code'],
			connectable: [],
			updatable: [],
			unavailable: ['GitHub Copilot CLI']
		});
		expect(note(availability(sources, 'usage'))).toBe('Not available for GitHub Copilot CLI.');
		expect(note(availability(sources, 'tools'))).toBeNull();
	});

	it('offers the optional layer that would provide it', () => {
		const a = availability([hooks({ connected: true }), telemetry(), copilot()], 'cost');
		expect(a.connectable).toEqual([
			{ tool: 'Claude Code', toolId: 'claude-code', layer: 'Telemetry' }
		]);
		expect(a.unavailable).toEqual([]); // Copilot is not in use: not mentioned
	});

	it('looks at every tool on first run, when none is in use', () => {
		const a = availability([hooks(), telemetry(), copilot()], 'model');
		expect(a.connectable.map((c) => c.tool)).toEqual(['Claude Code']);
		expect(a.unavailable).toEqual(['GitHub Copilot CLI']);
	});
});

describe('names', () => {
	it('joins tool names in a sentence', () => {
		expect([names([]), names(['A']), names(['A', 'B']), names(['A', 'B', 'C'])]).toEqual([
			'',
			'A',
			'A and B',
			'A, B and C'
		]);
	});
});

describe('toolOf', () => {
	it('finds the tool of any of its layers, and who answers in it', () => {
		const sources = [hooks(), telemetry(), copilot({ assistant: '' })];
		expect(toolOf(sources, 'claude-code-otel')?.id).toBe('claude-code');
		expect(assistantOf(toolOf(sources, 'claude-code'))).toBe('Claude');
		expect(assistantOf(toolOf(sources, 'copilot-cli'))).toBe('Agent');
		expect(assistantOf(toolOf(sources, 'gone'))).toBe('Agent');
	});

	it('says usage is not available for a tool without a layer that reports it', () => {
		const codex = source({
			id: 'codex',
			name: 'OpenAI Codex CLI',
			capabilities: ['prompt', 'model']
		});
		const tool = toolOf([hooks(), telemetry(), codex], 'codex')!;
		expect(note(availability(tool.layers, 'usage'))).toBe('Not available for OpenAI Codex CLI.');
	});

	it('says which version adds a capability the installed version lacks', () => {
		const codex = source({
			id: 'codex',
			name: 'OpenAI Codex CLI',
			connected: true,
			capabilities: ['prompt', 'model'],
			needs_version: { usage: '0.164.0' }
		});
		const a = availability([hooks({ connected: true }), codex], 'usage');
		expect(a.updatable).toEqual([{ tool: 'OpenAI Codex CLI', version: '0.164.0' }]);
		expect(a.unavailable).toEqual(['Claude Code']);
		expect(note(a)).toBe('Not available for Claude Code. Needs OpenAI Codex CLI 0.164.0 or later.');
	});
});
