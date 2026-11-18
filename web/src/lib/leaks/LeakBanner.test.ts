import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { LeakPlace } from '#lib/api/types.ts';
import { LeakDecider } from '#lib/leaks/decide.svelte.ts';
import { HideValues } from '#lib/leaks/hide.svelte.ts';
import LeakActions from '#lib/leaks/LeakActions.svelte';
import TurnLeaks from '#lib/leaks/TurnLeaks.svelte';
import SecretWarning from '#lib/prompts/SecretWarning.svelte';
import { leakOut, leaks } from '#lib/testing/builders.ts';

const NOW = new Date('2026-11-18T12:00:00Z');
const key = leakOut({ id: 'k', kind: 'Anthropic API key', masked: 'sk-…a91f', place: 'prompt' });

/** A decider that saves nothing (the flows are tested in decide.svelte.test.ts). */
const decider = () =>
	new LeakDecider('k', {
		set: (id, state) => Promise.resolve({ id, state, updated: 0 }),
		hideFlow: (findings) =>
			new HideValues({
				findings,
				load: () => Promise.resolve(leaks({ open: [key] })),
				hide: (id) => Promise.resolve({ id, state: 'hidden', updated: 1 })
			}),
		count: () => {}
	});

const text = (html: string) => html.replace(/<!--.*?-->/g, '').replace(/\s+/g, ' ');

describe('the banner of a possible leak', () => {
	it('names the kind, shows the value masked and offers the two actions, nothing else', () => {
		const out = text(render(SecretWarning, { props: { leak: key } }).body);
		expect(out).toContain('Possible leak: Anthropic API key');
		expect(out).toContain('sk-…a91f</code> in your prompt');
		expect(out).toContain('>Hide</');
		expect(out).toContain('>Not sensitive</');
		expect(out.match(/<button/g)).toHaveLength(2);
	});

	it.each([
		['prompt', 'in your prompt'],
		['answer', 'in the answer'],
		['tool_input', 'in a tool’s input'],
		['tool_output', 'in a tool’s output']
	] satisfies [LeakPlace, string][])('says where it sits: %s', (place, words) => {
		const out = text(render(SecretWarning, { props: { leak: leakOut({ place }) } }).body);
		expect(out).toContain(words);
	});

	it('becomes one quiet line once hidden, without actions', () => {
		const leak = leakOut({ decided: 'hidden', decided_at: '2026-11-16T09:00:00Z' });
		const out = text(render(SecretWarning, { props: { leak, now: NOW } }).body);
		expect(out).toContain('Hidden on 16 Nov');
		expect(out).not.toContain('<button');
	});
});

describe('LeakActions', () => {
	it('asks before hiding: the confirm stands in for the buttons', async () => {
		const d = decider();
		d.ask();
		await d.asking?.load();
		const out = text(render(LeakActions, { props: { id: 'k', decider: d } }).body);
		expect(out).toContain('Hide this value?');
		expect(out).toContain('cannot be undone');
		expect(out).not.toContain('Not sensitive');
	});

	it('offers Undo alone for a dismissed finding', () => {
		const out = text(render(LeakActions, { props: { id: 'k', dismissed: true } }).body);
		expect(out).toContain('>Undo</');
		expect(out).not.toContain('Not sensitive');
	});
});

describe('the leaks of a turn in the reader', () => {
	const board = leaks({
		open: [key, leakOut({ id: 'other', prompt: 9, masked: 'elsewhere' })],
		hidden: [leakOut({ id: 'h', masked: 'gone', decided_at: '2026-11-16T09:00:00Z' })],
		dismissed: [leakOut({ id: 'd', masked: 'dismissed-value' })]
	});

	it('shows a banner per open finding of the prompt, a line per hidden one, nothing dismissed', () => {
		const out = text(render(TurnLeaks, { props: { leaks: board, prompt: 1, now: NOW } }).body);
		expect(out).toContain('sk-…a91f');
		expect(out).toContain('Hidden on 16 Nov');
		expect(out).not.toContain('elsewhere');
		expect(out).not.toContain('dismissed-value');
	});

	it('shows nothing while the leaks are on their way', () => {
		const out = text(render(TurnLeaks, { props: { leaks: null, prompt: 1 } }).body);
		expect(out.trim()).toBe('');
	});
});
