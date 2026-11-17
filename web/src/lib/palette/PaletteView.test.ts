import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import { recent, results, hit } from '#lib/palette/fixtures.ts';
import { wordsFor } from '#lib/palette/mode.ts';
import { parseQuery } from '#lib/palette/query.ts';
import PaletteView from '#lib/palette/PaletteView.svelte';
import { buildView, type View } from '#lib/palette/rows.ts';
import { danglingReferences, duplicateIds, tagsWithRole } from '#lib/testing/aria.ts';
import { READY, type PanelState } from '#lib/ui/panelState.ts';

type Props = { query?: string; view: View; active?: number; status?: PanelState };

function draw({ query = '', view, active = 0, status = READY }: Props) {
	const noop = () => {};
	return render(PaletteView, {
		props: {
			query,
			view,
			active,
			status,
			keys: [
				{ key: '↵', label: 'open' },
				{ key: '⌘↵', label: 'copy' }
			],
			queryText: parseQuery(query).text,
			commandMode: parseQuery(query).mode === 'commands',
			onkeydown: noop,
			oninput: noop,
			onpick: noop,
			onhover: noop
		}
	}).body;
}

const withRows = () => buildView(parseQuery('rust'), recent, true, []);

describe('the palette in recall mode', () => {
	it('names the job in its own words, and has no command prefix to explain', () => {
		const html = render(PaletteView, {
			props: {
				query: '',
				view: { groups: [], noMatch: false },
				active: -1,
				status: READY,
				keys: [{ key: '↵', label: 'copy' }],
				queryText: '',
				words: wordsFor('recall'),
				onkeydown: () => {},
				oninput: () => {},
				onpick: () => {},
				onhover: () => {}
			}
		}).body;
		expect(html).toContain('placeholder="Find a prompt to copy"');
		expect(html).toContain('No prompt captured yet.');
		expect(html).toContain('<kbd class="svelte-lr5bpx">↵</kbd> copy');
		expect(html).not.toContain('for commands');
	});
});

describe('the palette as a screen reader meets it', () => {
	it('is a combobox over a listbox, with a name, and every reference resolves', () => {
		const html = draw({ query: 'rust', view: withRows() });
		const [combobox] = tagsWithRole(html, 'combobox');
		expect(combobox).toContain('aria-label="');
		expect(combobox).toContain('aria-expanded="true"');
		expect(combobox).toContain('aria-controls="palette-list"');
		expect(combobox).toContain('aria-activedescendant="palette-option-0"');
		expect(tagsWithRole(html, 'listbox')).toHaveLength(1);
		expect(danglingReferences(html)).toEqual([]);
		expect(duplicateIds(html)).toEqual([]);
	});

	it('gives every option an id, and marks exactly the active one selected', () => {
		const html = draw({ query: 'rust', view: withRows(), active: 1 });
		const options = tagsWithRole(html, 'option');
		expect(options).toHaveLength(3);
		for (const tag of options) expect(tag).toMatch(/id="palette-option-\d"/);
		expect(options.filter((t) => t.includes('aria-selected="true"'))).toHaveLength(1);
		expect(options[1]).toContain('aria-selected="true"');
	});

	it('labels each group of options', () => {
		const html = draw({ query: 'rust', view: withRows() });
		const groups = tagsWithRole(html, 'group');
		expect(groups).toHaveLength(3);
		for (const tag of groups) expect(tag).toContain('aria-labelledby="palette-group-');
		expect(html).toContain('Prompts');
	});

	it('draws matched words in bold as text, never as markup', () => {
		const dangerous = results({
			prompts: [
				hit({
					target: { kind: 'prompt', id: 1 },
					snippet: [
						{ text: '<img src=x onerror=alert(1)>', hit: true },
						{ text: ' <b>plain</b>', hit: false }
					]
				})
			]
		});
		const html = draw({ query: 'img', view: buildView(parseQuery('img'), dangerous, true, []) });
		expect(html).toMatch(/<b[^>]*>&lt;img src=x onerror=alert\(1\)><\/b>/);
		expect(html).toContain('&lt;b>plain&lt;/b>');
		expect(html).not.toContain('<img');
	});

	it('shows the keys of the highlighted row in the footer', () => {
		const html = draw({ view: withRows() });
		expect(html).toMatch(/<kbd[^>]*>↵<\/kbd> open/);
		expect(html).toMatch(/<kbd[^>]*>⌘↵<\/kbd> copy/);
		expect(html).toContain('close');
	});

	it('is a collapsed combobox with no listbox when nothing matches, and says so', () => {
		const html = draw({ query: 'zzz', view: { groups: [], noMatch: false } });
		expect(html).toContain('aria-expanded="false"');
		expect(html).not.toContain('role="listbox"');
		expect(danglingReferences(html)).toEqual([]);
		expect(html).toContain('Nothing matches “zzz”.');
	});

	it('says no command matched after >', () => {
		const html = draw({ query: '> zzz', view: { groups: [], noMatch: false } });
		expect(html).toContain('No command matches “zzz”.');
	});

	it('says what is happening instead of the rows while it searches or fails', () => {
		const loading = draw({
			view: { groups: [], noMatch: false },
			status: { kind: 'loading', label: 'Searching', body: 'rows', rows: 3 }
		});
		expect(loading).toContain('role="status"');
		expect(loading).toContain('Searching');
		const failed = draw({
			view: { groups: [], noMatch: false },
			status: { kind: 'error', error: new Error('down'), what: 'the search' }
		});
		expect(failed).toContain('role="alert"');
	});

	it('offers the Prompts page with the words when nothing matches', () => {
		const view = buildView(parseQuery('no such'), results(), true, []);
		const html = draw({ query: 'no such', view });
		expect(html).toContain('Nothing matches “no such”.');
		expect(html).toContain('Search the Prompts page for “no such”');
	});
});
