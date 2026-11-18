import { createRawSnippet, type ComponentProps } from 'svelte';
import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { WorthItem } from '#lib/api/types.ts';
import FirstScreen from '#lib/overview/FirstScreen.svelte';
import NeedsYou from '#lib/overview/NeedsYou.svelte';
import WorthALook from '#lib/overview/WorthALook.svelte';
import { attention, pickUp, source, worthItem } from '#lib/testing/builders.ts';

const NOW = Date.parse('2026-11-05T12:00:00Z');
const needs = (props: ComponentProps<typeof NeedsYou>) =>
	render(NeedsYou, { props: { now: NOW, ...props } }).body;
const worth = (items: WorthItem[]) => render(WorthALook, { props: { items } }).body;

const leak = worthItem({
	kind: 'possible_leak',
	text: '2 possible leaks in your prompts.',
	action: { type: 'hide_values', findings: ['f1', 'f2'] },
	target: { page: 'leaks' }
});
const redone = worthItem({
	kind: 'redone_turn',
	text: 'A failed turn cost $4.',
	action: { type: 'open_turn', prompt_id: 12 },
	target: { page: 'prompt', id: 12 }
});
const repeated = worthItem();

describe('NeedsYou: the four states', () => {
	it('ready: Pick up and Worth a look, side by side', () => {
		const out = needs({ attention: attention({ pick_up: [pickUp()], worth_a_look: [leak] }) });
		expect(out.indexOf('Pick up')).toBeGreaterThan(-1);
		expect(out.indexOf('Worth a look')).toBeGreaterThan(out.indexOf('Pick up'));
		expect(out).toContain('class="lists');
		expect(out).not.toContain('Nothing needs you');
	});

	it('loading: a skeleton for each list', () => {
		const out = needs({});
		expect(out).toContain('Loading the sessions to pick up');
		expect(out).toContain('Loading what is worth a look');
	});

	it('nothing in range: "Nothing needs you", with the period in one line', () => {
		const calm = 'Last 7 days: 278 prompts · $453 · 181 commits';
		const out = needs({ attention: attention(), calm });
		expect(out).toContain('Nothing needs you');
		expect(out).toContain(calm);
		expect(out).not.toContain('Worth a look');
	});

	it('keeps an empty list beside a full one, saying so', () => {
		const out = needs({ attention: attention({ worth_a_look: [repeated] }) });
		expect(out).toContain('Nothing to pick up');
	});
});

describe('FirstScreen', () => {
	const connected = [source({ connected: true, total: 12 })];
	const rest = createRawSnippet(() => ({ render: () => '<div id="key-figures">figures</div>' }));

	it('first run: the setup steps, no lists', () => {
		const out = render(FirstScreen, { props: { sources: [source()], children: rest } }).body;
		expect(out).toContain('id="setup-title"');
		expect(out).not.toContain('Pick up');
	});

	it('puts the two lists first, before the key figures and the panels', () => {
		const props = {
			sources: connected,
			attention: attention({ pick_up: [pickUp()], worth_a_look: [repeated] }),
			children: rest
		};
		const out = render(FirstScreen, { props }).body;
		const pick = out.indexOf('Pick up');
		const look = out.indexOf('Worth a look');
		expect(pick).toBeGreaterThan(-1);
		expect(look).toBeGreaterThan(pick);
		expect(out.indexOf('id="key-figures"')).toBeGreaterThan(look);
		expect(out).not.toContain('id="setup-title"');
		// Compact: no fixed height that could push the lists below the fold.
		expect(out).not.toMatch(/style="[^"]*height/);
	});
});

describe('WorthALook', () => {
	it('keeps the daemon order, one sentence and one button each', () => {
		const out = worth([leak, redone, repeated]);
		const at = [leak, redone, repeated].map((i) => out.indexOf(i.text));
		expect(at).toEqual([...at].sort((a, b) => a - b));
		expect(out).toContain('>Hide values');
		expect(out).toContain('href="/prompts?id=12"');
		expect(out).toContain('Open the turn');
		expect(out).toContain('Copy as command');
		expect(out).toContain('.claude/commands/run-tests.md');
	});

	it('asks nothing before Hide values is pressed', () => {
		expect(worth([leak])).not.toContain('cannot be undone');
	});

	it('shows a kind it does not know as a plain link to its target', () => {
		const item = Object.assign(
			worthItem({ text: 'Something new.', target: { page: 'session', id: 's9' } }),
			{
				kind: 'stale_branch'
			}
		);
		const out = worth([item]);
		expect(out).toContain('Something new.');
		expect(out).toMatch(/<a class="plain[^"]*" href="\/sessions\/s9">Have a look<\/a>/);
		expect(out).not.toContain('<button');
	});

	it('says when nothing stands out', () => {
		expect(worth([])).toContain('Nothing stands out');
	});
});
