import { describe, expect, it } from 'vitest';
import { NOW, STATE_FIXTURES } from '#lib/sources/fixtures.ts';
import {
	QUIET_AFTER_MS,
	STATE_IDS,
	STATES,
	connectorState,
	explain,
	layerName,
	present,
	promptsQuery,
	tools
} from '#lib/sources/state.ts';
import { source } from '#lib/testing/builders.ts';

describe('connectorState', () => {
	it.each(STATE_IDS)('reaches %s from its fixture', (id) => {
		expect(connectorState(STATE_FIXTURES[id], NOW)).toBe(id);
	});

	it('has a fixture for every state and no more', () => {
		expect(Object.keys(STATE_FIXTURES).sort()).toEqual([...STATE_IDS].sort());
		expect(Object.keys(STATES).sort()).toEqual([...STATE_IDS].sort());
	});

	it('turns quiet only after a full day without events', () => {
		const at = (ms: number) =>
			connectorState(source({ connected: true, last_at: new Date(NOW - ms).toISOString() }), NOW);
		expect(at(QUIET_AFTER_MS)).toBe('receiving');
		expect(at(QUIET_AFTER_MS + 1)).toBe('quiet');
	});

	it('reports a turned-off connector before anything else', () => {
		const off = { enabled: false };
		expect(connectorState(source({ ...off, detected: false }), NOW)).toBe('disabled');
		expect(connectorState(source({ ...off, connected: true, blocks: ['b'] }), NOW)).toBe(
			'disabled'
		);
		expect(explain('disabled', source({ ...off, connected: true }), NOW)).toContain('disconnect');
	});

	it('reports policy before anything else once the tool is installed', () => {
		const blocks = ['disableAllHooks is set in managed-settings.json'];
		expect(connectorState(source({ blocks, warnings: ['w'] }), NOW)).toBe('blocked_by_policy');
		expect(connectorState(source({ blocks, connected: true, outdated: true }), NOW)).toBe(
			'blocked_by_policy'
		);
		expect(connectorState(source({ blocks, detected: false }), NOW)).toBe('not_installed');
	});

	it('ranks an update over drift, and drift over activity', () => {
		const connected = { connected: true, last_at: new Date(NOW).toISOString() };
		expect(connectorState(source({ ...connected, outdated: true, unmapped: 2 }), NOW)).toBe(
			'outdated'
		);
		expect(connectorState(source({ ...connected, unmapped: 2 }), NOW)).toBe('drift');
	});

	it('keeps a connected connector out of not_installed and warning', () => {
		expect(connectorState(source({ connected: true, detected: false }), NOW)).toBe('waiting');
		expect(connectorState(source({ connected: true, warnings: ['w'] }), NOW)).toBe('waiting');
	});
});

describe('presentation', () => {
	it.each(STATE_IDS)('%s has a label, an icon, an action and one sentence', (id) => {
		const p = present(STATE_FIXTURES[id], NOW);
		expect(p.id).toBe(id);
		expect(p.label).toBeTruthy();
		expect(p.icon).toBeTruthy();
		expect(p.actionLabel).toBeTruthy();
		expect(p.explanation).toMatch(/^[A-Z0-9].*[.]$/);
		expect(p.explanation.split(/\.\s/)).toHaveLength(1);
	});

	it('names labels uniquely, so the words alone tell states apart', () => {
		const labels = STATE_IDS.map((id) => STATES[id].label);
		expect(new Set(labels).size).toBe(labels.length);
	});

	it('states counts and times in its sentence', () => {
		expect(explain('drift', STATE_FIXTURES.drift, NOW)).toContain('3 prompts');
		expect(explain('quiet', STATE_FIXTURES.quiet, NOW)).toContain('yesterday');
		expect(explain('available', STATE_FIXTURES.available, NOW)).toContain(
			'~/.claude/settings.json'
		);
	});

	it('says what is left to do in the tool while waiting or quiet', () => {
		const step = 'Open Codex, type /hooks and trust the hooks.';
		const waiting = source({ connected: true, after_connect: step });
		expect(explain('waiting', waiting, NOW)).toBe(
			`Connected. ${step} Then events arrive once you start a new session and send a prompt.`
		);
		expect(explain('quiet', { ...STATE_FIXTURES.quiet, after_connect: step }, NOW)).toContain(step);
		expect(explain('waiting', STATE_FIXTURES.waiting, NOW)).not.toContain('Then');
	});
});

describe('tools', () => {
	const hooks = source();
	const telemetry = source({ id: 'claude-code-otel', part_of: 'claude-code', layer: 'Telemetry' });

	it('groups layers under their main connector, the main one first', () => {
		expect(tools([telemetry, hooks])).toEqual([
			{ id: 'claude-code', name: 'Claude Code', layers: [hooks, telemetry] }
		]);
	});

	it('shows a layer whose main connector is missing as a tool of its own', () => {
		expect(tools([telemetry]).map((t) => t.layers)).toEqual([[telemetry]]);
	});

	it('names a layer by its layer name, else its own name', () => {
		expect(layerName(telemetry)).toBe('Telemetry');
		expect(layerName(source({ layer: '', name: 'Cursor' }))).toBe('Cursor');
	});

	it('links to a connector’s own prompts only when it has some', () => {
		expect(promptsQuery(source({ total: 3 }))).toBe('?source=claude-code');
		expect(promptsQuery(telemetry)).toBe('');
	});
});
