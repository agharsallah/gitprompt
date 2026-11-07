import { describe, expect, it } from 'vitest';
import { availability, names, note, toolCapabilities } from '#lib/sources/capabilities.ts';
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
