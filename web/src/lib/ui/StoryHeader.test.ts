import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import StoryHeader from '#lib/ui/StoryHeader.svelte';

const draw = (headline: string | null) =>
	render(StoryHeader, { props: { title: 'Overview', headline, totals: '3 prompts · 1 session' } })
		.body;

describe('StoryHeader', () => {
	it('shows the totals line alone when nothing stands out', () => {
		const html = draw(null);
		expect(html).toContain('3 prompts · 1 session');
		expect(html).not.toContain('class="totals');
	});

	it('leads with the sentence, the totals line under it', () => {
		const html = draw('Spend doubled this week');
		expect(html.indexOf('3 prompts · 1 session')).toBeGreaterThan(
			html.indexOf('Spend doubled this week')
		);
	});
});
