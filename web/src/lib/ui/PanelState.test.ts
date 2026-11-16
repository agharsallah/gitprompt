import { createRawSnippet } from 'svelte';
import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import Panel from '#lib/ui/Panel.svelte';
import { readyOr, type PanelState as State } from '#lib/ui/panelState.ts';
import PanelState from '#lib/ui/PanelState.svelte';

const content = createRawSnippet(() => ({ render: () => '<p>the content</p>' }));

/** The HTML of the component for a state, as the server build draws it. */
const html = (state: State) => render(PanelState, { props: { state, children: content } }).body;

describe('PanelState', () => {
	it('draws the children alone when ready', () => {
		expect(html({ kind: 'ready' }).replace(/<!--.*?-->/g, '')).toBe('<p>the content</p>');
	});

	it('draws a skeleton and announces the loading, not the content', () => {
		const out = html({ kind: 'loading', label: 'Loading the sample', body: 'rows', rows: 3 });
		expect(out).toContain('role="status"');
		expect(out).toContain('Loading the sample');
		expect(out).not.toContain('the content');
	});

	it('says what an empty panel means, and the next step when there is one', () => {
		const out = html({
			kind: 'empty',
			title: 'No starred prompts here',
			message: 'Star a prompt first.',
			next: { label: 'Open the prompts', href: '/prompts' }
		});
		expect(out).toContain('No starred prompts here');
		expect(out).toContain('Star a prompt first.');
		expect(out).toContain('href="/prompts"');
		expect(out).toContain('Open the prompts');
		expect(out).not.toContain('the content');
	});

	it('draws an empty state without a body or an action when it has neither', () => {
		const out = html({ kind: 'empty', title: 'No sessions in this window' });
		expect(out).toContain('No sessions in this window');
		expect(out).not.toContain('class="body"');
		expect(out).not.toContain('class="actions"');
	});

	it('says what is not reported, and the way to get it, like an empty state', () => {
		const out = html({
			kind: 'unsupported',
			title: 'Costs not available',
			message: 'Costs come from Claude Code’s hooks.',
			next: { label: 'Connect Claude Code', href: '/sources#tool-claude-code', title: 'Connectors' }
		});
		expect(out).toContain('Costs not available');
		expect(out).toContain('Costs come from Claude Code’s hooks.');
		expect(out).toContain('href="/sources#tool-claude-code"');
		expect(out).not.toContain('the content');
	});

	it('says an error the usual way, with a retry when given one', () => {
		const out = html({
			kind: 'error',
			error: new TypeError('Failed to fetch'),
			what: 'this panel',
			retry: () => {}
		});
		expect(out).toContain('GitPrompt is not running');
		expect(out).toContain('Try again');
		expect(out).not.toContain('the content');
	});

	it('offers no retry without one', () => {
		const out = html({
			kind: 'error',
			error: new TypeError('Failed to fetch'),
			what: 'this panel'
		});
		expect(out).toContain('GitPrompt is not running');
		expect(out).not.toContain('Try again');
	});

	it('shows the content under a live chip when there is new data', () => {
		const out = html({ kind: 'fresh', count: 3, unit: 'prompt' });
		expect(out).toContain('role="status"');
		expect(out).toContain('3 new prompts');
		expect(out).toContain('the content');
	});

	it('agrees the chip with a single new item and works without a unit', () => {
		expect(html({ kind: 'fresh', count: 1, unit: 'prompt' })).toContain('1 new prompt<');
		expect(html({ kind: 'fresh', count: 2 })).toContain('2 new<');
	});

	it('says what a sparse panel needs, and what there is so far', () => {
		const out = html({ kind: 'sparse', minimum: 10, unit: 'prompt', have: 4 });
		expect(out).toContain('Too little data yet');
		expect(out).toContain('at least 10 prompts');
		expect(out).toContain('there are 4 so far');
		expect(out).not.toContain('the content');
	});

	it('says a sparse panel needs one prompt without a count so far', () => {
		const out = html({ kind: 'sparse', minimum: 1 });
		expect(out).toContain('at least 1 prompt to');
		expect(out).not.toContain('so far');
	});
});

describe('Panel', () => {
	const panel = (state?: State) =>
		render(Panel, { props: { title: 'Sample', state, children: content } }).body;

	it('keeps its title and draws the content when ready, by default', () => {
		for (const out of [panel(), panel({ kind: 'ready' })]) {
			expect(out).toContain('Sample');
			expect(out).toContain('the content');
		}
	});

	it('keeps its title and replaces the content with the state', () => {
		const out = panel({ kind: 'empty', title: 'Nothing here' });
		expect(out).toContain('Sample');
		expect(out).toContain('Nothing here');
		expect(out).not.toContain('the content');
	});
});

describe('readyOr', () => {
	const empty = { kind: 'empty', title: 'Nothing' } as const;

	it('is ready when there is something to show', () => {
		expect(readyOr(3, empty)).toEqual({ kind: 'ready' });
	});

	it('is the empty state when there is not', () => {
		expect(readyOr(0, empty)).toBe(empty);
		expect(readyOr([].length, empty)).toBe(empty);
	});
});
