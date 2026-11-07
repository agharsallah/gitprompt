import { describe, expect, it } from 'vitest';
import type { Availability } from '#lib/sources/capabilities.ts';
import { facts } from '#lib/testing/builders.ts';
import { factsView, missingTokens } from '#lib/prompts/facts.ts';

const codex = { id: 'codex', name: 'OpenAI Codex CLI', label: 'Codex', assistant: 'Codex' };
const usage = (over: Partial<Availability>): Availability => ({
	capability: 'usage',
	available: [],
	connectable: [],
	updatable: [],
	unavailable: [],
	...over
});

describe('factsView', () => {
	it('names the tool and the model the hooks reported, without tokens or cost', () => {
		const view = factsView(facts({ connector: codex, model: 'gpt-6-luna', model_source: 'hook' }));
		expect(view.tool).toEqual({ label: 'Codex', name: 'OpenAI Codex CLI' });
		expect(view.assistant).toBe('Codex');
		expect(view.model).toEqual({
			id: 'gpt-6-luna',
			name: 'GPT-6 Luna',
			hint: 'Model the tool reported with the prompt: gpt-6-luna'
		});
		expect(view.tokens).toBeNull();
		expect(view.cost).toBeNull(); // not "$0"
	});

	it('shows reported tokens with their breakdown, and a cost only when reported', () => {
		const tokens = {
			input: 4076,
			output: 14,
			cache_read: 10752,
			cache_creation: 0,
			reasoning: 0,
			total: 14842
		};
		const view = factsView(facts({ connector: codex, model: 'gpt-6-luna', tokens }));
		expect(view.tokens?.total).toBe('14.8K');
		expect(view.tokens?.hint).toBe(
			'14,842 tokens: 4,076 input, 14 output, 10,752 read from the cache'
		);
		expect(view.cost).toBeNull();
		expect(factsView(facts({ tokens, cost_usd: 0.42 })).cost).toBe('$0.42');
		expect(factsView(facts({ tokens, cost_usd: 0 })).cost).toBe('$0'); // reported, and free
	});

	it('falls back to "Agent" when the tool names no assistant', () => {
		expect(
			factsView(facts({ connector: { id: 'x', name: 'x', label: 'x', assistant: '' } })).assistant
		).toBe('Agent');
	});
});

describe('missingTokens', () => {
	const none = facts({ connector: codex });
	it('says nothing when tokens are shown, or when the tool is unknown', () => {
		const tokens = {
			input: 1,
			output: 1,
			cache_read: 0,
			cache_creation: 0,
			reasoning: 0,
			total: 2
		};
		expect(missingTokens(facts({ tokens }), usage({ unavailable: ['Codex'] }))).toBeNull();
		expect(missingTokens(none, undefined)).toBeNull();
	});
	it('names the version that adds tokens', () => {
		const a = usage({ updatable: [{ tool: 'OpenAI Codex CLI', version: '0.164.0' }] });
		expect(missingTokens(none, a)).toBe('Needs OpenAI Codex CLI 0.164.0 or later.');
	});
	it('names the layer to connect', () => {
		const a = usage({
			connectable: [{ tool: 'Claude Code', toolId: 'claude-code', layer: 'Telemetry' }]
		});
		expect(missingTokens(none, a)).toBe('Connect Telemetry to see tokens and cost.');
	});
	it('says the tool cannot report them', () => {
		expect(missingTokens(none, usage({ unavailable: ['GitHub Copilot CLI'] }))).toBe(
			"GitHub Copilot CLI doesn't report tokens."
		);
	});
	it('says when a tool that reports tokens reported none for this turn', () => {
		expect(missingTokens(none, usage({ available: ['OpenAI Codex CLI'] }))).toBe(
			'No token counts were reported for this turn.'
		);
	});
});
