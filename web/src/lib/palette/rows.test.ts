import { describe, expect, it } from 'vitest';
import { hit, recent, results } from '#lib/palette/fixtures.ts';
import type { Command } from '#lib/palette/commands.ts';
import { parseQuery } from '#lib/palette/query.ts';
import { buildView, firstIndexes, rowsOf } from '#lib/palette/rows.ts';

const run = () => 0;
const commandsList: Command[] = [
	{ id: 'a', title: 'Export this view', keywords: [], run },
	{ id: 'b', title: 'Toggle theme', keywords: ['dark'], run }
];
const view = (raw: string, found = recent, final = true) =>
	buildView(parseQuery(raw), found, final, commandsList);

describe('the rows of the palette', () => {
	it('opens on your last sessions, then prompts, then projects', () => {
		expect(view('').groups.map((g) => g.kind)).toEqual(['session', 'prompt', 'project']);
	});

	it('puts the prompts first once words are typed', () => {
		expect(view('rust').groups.map((g) => g.kind)).toEqual(['prompt', 'session', 'project']);
	});

	it('shows a session by its name, a prompt by its words with its session beside', () => {
		const rows = rowsOf(view('').groups);
		const session = rows.find((r) => r.glyph === 'session');
		const prompt = rows.find((r) => r.glyph === 'prompt');
		expect(session?.title.map((s) => s.text).join('')).toBe('Port the store');
		expect(session?.item).toEqual({ kind: 'session', id: 's-1' });
		expect(prompt?.title).toEqual(recent.prompts[0].snippet);
		expect(prompt?.context).toBe('Block flaky timing');
		expect(prompt?.item).toEqual({ kind: 'prompt', id: 7 });
		expect(prompt?.at).toBe('2026-11-16T10:00:00Z');
	});

	it('marks the typed words in a name', () => {
		const row = rowsOf(view('port').groups).find((r) => r.glyph === 'session');
		expect(row?.title[0]).toEqual({ text: 'Port', hit: true });
	});

	it('names a session that has no name', () => {
		const found = results({ sessions: [hit({ target: { kind: 'session', id: 'x' } })] });
		expect(rowsOf(view('', found).groups)[0].title[0].text).toBe('Untitled session');
	});

	it('narrows to projects after # and to sessions after @', () => {
		expect(view('#git').groups.map((g) => g.kind)).toEqual(['project']);
		expect(view('@port').groups.map((g) => g.kind)).toEqual(['session']);
	});

	it('lists the registered commands after >, filtered by the words', () => {
		expect(rowsOf(view('>').groups).map((r) => r.key)).toEqual(['command:a', 'command:b']);
		expect(rowsOf(view('> dark').groups).map((r) => r.key)).toEqual(['command:b']);
		expect(view('> zzz')).toEqual({ groups: [], noMatch: false });
	});

	it('says nothing matched, and offers the Prompts page with the words', () => {
		const out = view('no such thing', results());
		expect(out.noMatch).toBe(true);
		const [row] = rowsOf(out.groups);
		expect(row.item.kind).toBe('page');
		expect(row.item.kind === 'page' && row.item.href).toContain('/prompts?q=no%20such%20thing');
	});

	it('does not say it before the answer to these words is known', () => {
		expect(view('rust', results(), false)).toEqual({ groups: [], noMatch: false });
		expect(buildView(parseQuery('rust'), undefined, true, [])).toEqual({
			groups: [],
			noMatch: false
		});
	});

	it('is empty for an empty query with nothing captured, without a way out', () => {
		expect(view('', results())).toEqual({ groups: [], noMatch: false });
	});

	it('counts where each group starts among all rows', () => {
		const { groups } = view('');
		expect(firstIndexes(groups)).toEqual([0, 1, 2]);
		expect(rowsOf(groups)).toHaveLength(3);
	});
});
