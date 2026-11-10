import { describe, expect, it } from 'vitest';
import { NOW, STATE_FIXTURES } from '#lib/sources/fixtures.ts';
import {
	dismissValue,
	isDismissed,
	pendingUpdates,
	updateQuery,
	updateSummary
} from '#lib/sources/updates.ts';
import { source } from '#lib/testing/builders.ts';

const outdated = (overrides = {}) => source({ connected: true, outdated: true, ...overrides });

describe('pendingUpdates', () => {
	it('lists only connectors in the outdated state', () => {
		const states = Object.values(STATE_FIXTURES).map((s, i) => ({ ...s, id: `s${i}` }));
		expect(pendingUpdates(states, NOW).map((u) => u.id)).toEqual([
			states.find((s) => s.outdated && s.connected)!.id
		]);
	});

	it('does not ask to update a connector that is turned off or blocked', () => {
		const sources = [
			outdated({ id: 'a', enabled: false }),
			outdated({ id: 'b', blocks: ['disableAllHooks is set'] })
		];
		expect(pendingUpdates(sources, NOW)).toEqual([]);
	});

	it('names a layer after its tool', () => {
		const sources = [
			source({ id: 'claude-code', name: 'Claude Code', connected: true }),
			outdated({
				id: 'claude-code-otel',
				name: 'Claude Code telemetry',
				part_of: 'claude-code',
				layer: 'Telemetry'
			})
		];
		expect(pendingUpdates(sources, NOW)).toEqual([
			{ id: 'claude-code-otel', name: 'Claude Code · Telemetry' }
		]);
	});
});

describe('the hint', () => {
	const one = [{ id: 'codex', name: 'Codex' }];
	const two = [...one, { id: 'cursor', name: 'Cursor' }];
	const three = [...two, { id: 'copilot-cli', name: 'GitHub Copilot CLI' }];

	it('opens the review of a single update, else the Connectors page', () => {
		expect(updateQuery(one)).toBe('?connect=codex');
		expect(updateQuery(two)).toBe('');
	});

	it('names up to two connections, then counts them', () => {
		expect(updateSummary(one)).toBe('Codex can capture more after an update.');
		expect(updateSummary(two)).toBe('Codex and Cursor can capture more.');
		expect(updateSummary(three)).toBe('3 connections can capture more after an update.');
	});

	it('stays dismissed while the set is unchanged or smaller, and comes back for a new update', () => {
		const stored = dismissValue(two);
		expect(isDismissed(two, stored)).toBe(true);
		expect(isDismissed(one, stored)).toBe(true);
		expect(isDismissed(three, stored)).toBe(false);
		expect(isDismissed(one, null)).toBe(false);
	});
});
