import { createRawSnippet } from 'svelte';
import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import InlineRename from '#lib/ui/InlineRename.svelte';

const onsave = () => Promise.resolve();
const html = (props: Record<string, unknown>) =>
	render(InlineRename, {
		props: { label: 'Session name', onsave, value: 'Fix the parser', ...props }
	}).body;

// Editing, saving and rolling back are the state's (inlineRename.state.test.ts); this is what shows first.
describe('InlineRename at rest', () => {
	it('shows the text as a button that starts the editing, named for what it edits', () => {
		const out = html({});
		expect(out).toMatch(/<button[^>]*type="button"[^>]*>Fix the parser/);
		expect(out).toContain('edit session name');
		expect(out).not.toContain('<input');
		expect(out).not.toContain('role="alert"');
	});

	it('draws the text itself and a separate edit button when given a display snippet', () => {
		const display = createRawSnippet<[string]>((text) => ({
			render: () => `<a href="/somewhere">${text()}</a>`
		}));
		const out = html({ display });
		expect(out).toContain('<a href="/somewhere">Fix the parser</a>');
		expect(out).toContain('aria-label="Edit session name"');
	});
});
