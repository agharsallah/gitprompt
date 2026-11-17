import { render } from 'svelte/server';
import { describe, expect, it, vi } from 'vitest';
import Palette from '#lib/palette/Palette.svelte';
import { PaletteState } from '#lib/palette/open.svelte.ts';
import { danglingReferences, tagsWithRole } from '#lib/testing/aria.ts';

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));
vi.mock('$app/state', () => ({ page: { url: new URL('http://x/') } }));

describe('the palette dialog in recall mode', () => {
	it('is a dialog named for the job, with its own combobox, closed until the window shows', () => {
		const html = render(Palette, { props: { palette: new PaletteState(), mode: 'recall' } }).body;
		const [dialog] = html.match(/<dialog[^>]*>/) ?? [];
		expect(dialog).toContain('aria-label="Find a prompt to copy"');
		expect(dialog).toContain('class="palette recall');
		expect(dialog).not.toMatch(/\bopen\b/);
		expect(tagsWithRole(html, 'combobox')).toHaveLength(1);
		expect(html).not.toContain('for commands');
		expect(danglingReferences(html)).toEqual([]);
	});
});

describe('the palette dialog', () => {
	it('is a named dialog around the combobox, closed until it is opened', () => {
		const html = render(Palette, { props: { palette: new PaletteState() } }).body;
		const [dialog] = html.match(/<dialog[^>]*>/) ?? [];
		expect(dialog).toContain('aria-label="Find or run a command"');
		expect(dialog).not.toMatch(/\bopen\b/);
		expect(tagsWithRole(html, 'combobox')).toHaveLength(1);
		expect(danglingReferences(html)).toEqual([]);
	});
});
