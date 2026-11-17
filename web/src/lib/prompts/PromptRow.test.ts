import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import PromptRow from '#lib/prompts/PromptRow.svelte';
import { prompt } from '#lib/testing/builders.ts';

const draw = (p = prompt({ id: 7 })) =>
	render(PromptRow, { props: { prompt: p, outcome: 'done', href: '/prompts?id=7' } }).body;

/** The row's link, up to its closing tag. */
const linkOf = (html: string) => html.slice(html.indexOf('<a '), html.indexOf('</a>'));

describe('a prompt row', () => {
	it('has a copy button next to the link, not inside it', () => {
		const html = draw();
		expect(html.match(/<button/g)).toHaveLength(1);
		expect(html).toContain('aria-label="Copy prompt"');
		expect(linkOf(html)).not.toContain('<button');
	});

	it('has no copy button when no text was captured', () => {
		expect(draw(prompt({ id: 8, prompt: null, typed_text: null }))).not.toContain('<button');
	});

	it('still names the prompt and where it opens', () => {
		const html = draw();
		expect(linkOf(html)).toContain('href="/prompts?id=7"');
		expect(linkOf(html)).toContain('Fix the failing test');
	});
});
