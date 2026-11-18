// The project page's tab lives in its URL: a reload opens the same tab, and Back returns to the
// tab before, because opening one pushes a history entry and the page reads the tab from the URL.
// Switching tabs never loads the story again: the page's load does not read the URL.
import { describe, expect, it, vi } from 'vitest';
import { commitAnchor } from '#lib/projects/commits.ts';
import { openTab, PROJECT_TABS, tabHref, tabOf } from '#lib/projects/tabs.ts';
import { projectHref } from '#lib/shared/links.ts';

const ID = 'L3dvcmsvYXBw';
const at = (href: string) => new URL(href, 'http://localhost');

describe('the project page’s tabs', () => {
	it.each(PROJECT_TABS.map((t) => t.id))('a reload of the %s tab opens it again', (tab) => {
		expect(tabOf(at(tabHref(ID, tab)))).toBe(tab);
	});

	it('opens on the journey, which keeps the URL bare', () => {
		expect(tabHref(ID, 'journey')).toBe(projectHref(ID));
		expect(tabOf(at(projectHref(ID)))).toBe('journey');
		expect(tabOf(at(projectHref(ID, { tab: 'nonsense' })))).toBe('journey');
	});

	it('opening a tab pushes a history entry, so Back returns to the tab before', () => {
		const go = vi.fn();
		openTab(ID, 'commits', go);
		openTab(ID, 'journey', go);
		expect(go.mock.calls).toEqual([
			[tabHref(ID, 'commits'), { replace: false, reset: false }],
			[tabHref(ID, 'journey'), { replace: false, reset: false }]
		]);
		// Back from the journey: the URL before it, read as the Commits tab.
		expect(tabOf(at(go.mock.calls[0][0]))).toBe('commits');
	});

	it('switching tabs keeps the range the page was opened on', () => {
		expect(tabHref(ID, 'commits', '24h')).toBe(projectHref(ID, { tab: 'commits', range: '24h' }));
		expect(tabHref(ID, 'journey', '24h')).toBe(projectHref(ID, { range: '24h' }));
		const go = vi.fn();
		openTab(ID, 'commits', go, '24h');
		expect(go).toHaveBeenCalledWith(tabHref(ID, 'commits', '24h'), {
			replace: false,
			reset: false
		});
	});

	it('a link to a commit’s row opens the Commits tab, even without naming it', () => {
		expect(tabOf(at(projectHref(ID, { anchor: commitAnchor('a9f0971') })))).toBe('commits');
		expect(tabOf(at(projectHref(ID, { anchor: 'prompt-7' })))).toBe('journey');
	});

	it('switching tabs does not load the story again: the load never reads the URL', async () => {
		const { load } = await import('./+page.ts');
		const url = new Proxy(at(tabHref(ID, 'commits')), {
			get: () => {
				throw new Error('the load read the URL');
			}
		});
		const fetch = () => Promise.reject(new Error('offline'));
		const event = { url, fetch, params: { id: ID } };
		expect(() => load(event as never)).not.toThrow();
	});
});
