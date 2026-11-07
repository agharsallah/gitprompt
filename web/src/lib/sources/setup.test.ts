import { describe, expect, it } from 'vitest';
import { connectQuery, setupSteps } from '#lib/sources/setup.ts';
import { source } from '#lib/testing/builders.ts';

const telemetry = (overrides = {}) =>
	source({
		id: 'claude-code-otel',
		part_of: 'claude-code',
		layer: 'Telemetry',
		summary: 'Model, tokens and cost.',
		...overrides
	});

describe('setupSteps', () => {
	it('connects the main layer, offers the optional ones, then asks for a prompt', () => {
		const steps = setupSteps([source(), telemetry()]);
		expect(steps.map((s) => [s.title, s.optional, s.connect])).toEqual([
			['Connect Claude Code', false, 'claude-code'],
			['Add telemetry', true, 'claude-code-otel'],
			['Send a prompt', false, undefined]
		]);
		expect(steps.every((s) => !s.done)).toBe(true);
	});

	it('checks steps off as connectors connect and the first prompt arrives', () => {
		const steps = setupSteps([source({ connected: true, total: 1 }), telemetry()]);
		expect(steps.map((s) => s.done)).toEqual([true, false, true]);
	});

	it('starts with an installed tool, and says when none is', () => {
		const cursor = source({ id: 'cursor', name: 'Cursor', layer: '' });
		const missing = source({ detected: false });
		expect(setupSteps([missing, cursor])[0].title).toBe('Connect Cursor');
		expect(setupSteps([missing])[0].body).toContain('Install Claude Code first');
	});

	it('has nothing to show without connectors', () => {
		expect(setupSteps([])).toEqual([]);
	});

	it('links a step to the connector’s consent review', () => {
		expect(connectQuery('claude-code')).toBe('?connect=claude-code');
	});
});
