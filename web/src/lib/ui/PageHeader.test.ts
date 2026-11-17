import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import PageHeader from '#lib/ui/PageHeader.svelte';

const draw = (props: { title: string; purpose?: string; description?: string }) =>
	render(PageHeader, { props }).body;

describe('PageHeader', () => {
	it('states the page’s purpose under its title, before the description', () => {
		const html = draw({ title: 'Prompts', purpose: 'Find a prompt again', description: 'Longer.' });
		const at = (text: string) => html.indexOf(text);
		expect(at('Prompts')).toBeGreaterThan(-1);
		expect(at('Find a prompt again')).toBeGreaterThan(at('</h1>'));
		expect(at('Longer.')).toBeGreaterThan(at('Find a prompt again'));
	});

	it('draws no purpose line without one', () => {
		expect(draw({ title: 'Settings' })).not.toContain('class="purpose');
	});
});
