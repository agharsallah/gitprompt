import { render } from 'svelte/server';
import { describe, expect, it, vi } from 'vitest';
import Sidebar from '#lib/layout/Sidebar.svelte';

vi.mock('$app/state', () => ({
	page: { url: new URL('http://localhost/prompts/'), route: { id: '/prompts' } }
}));

const html = () => render(Sidebar).body;

describe('the sidebar as drawn', () => {
	it('lists the everyday pages far to near, each with its purpose line', () => {
		const out = html();
		const order = ['Overview', 'Projects', 'Sessions', 'Prompts'].map((n) => out.indexOf(`>${n}<`));
		expect(order.every((at) => at > -1)).toBe(true);
		expect(order).toEqual([...order].sort((a, b) => a - b));
		for (const line of [
			'What needs you',
			'Where each project stands',
			'Get back into a session',
			'Find a prompt again'
		]) {
			expect(out).toContain(`>${line}<`);
		}
	});

	it('shows each purpose line as the link’s tooltip, for the collapsed sidebar', () => {
		expect(html()).toContain('title="Sessions: Get back into a session"');
	});

	it('marks the current page for assistive technology, once', () => {
		const out = html();
		expect(out.match(/aria-current="page"/g)).toHaveLength(1);
	});

	it('has a Find button with the shortcut, and no link to /search', () => {
		const out = html();
		expect(out).toContain('aria-label="Find anything"');
		expect(out).toMatch(/<kbd[^>]*>(⌘K|Ctrl K)<\/kbd>/);
		expect(out).not.toContain('/search');
	});
});
