import { describe, expect, it, vi } from 'vitest';
import { actionsFor, perform, type ActionContext } from '#lib/palette/actions.ts';
import { recent } from '#lib/palette/fixtures.ts';
import {
	actionKeyIn,
	actionsIn,
	copyThenDone,
	isRecall,
	kindsIn,
	parseFor,
	restrict,
	wordsFor
} from '#lib/palette/mode.ts';
import { parseQuery } from '#lib/palette/query.ts';
import { buildView, rowsOf } from '#lib/palette/rows.ts';

describe('which page is the recall window', () => {
	it('is the dashboard opened with ?recall=1, and nothing else', () => {
		expect(isRecall(new URL('http://127.0.0.1:7878/?recall=1'))).toBe(true);
		expect(isRecall(new URL('http://127.0.0.1:7878/'))).toBe(false);
		expect(isRecall(new URL('http://127.0.0.1:7878/?recall=0'))).toBe(false);
		expect(isRecall(new URL('http://127.0.0.1:7878/prompts/?range=7d'))).toBe(false);
	});
});

describe('what recall asks for', () => {
	it('searches prompts only, and reads a prefix as the words it is', () => {
		const parsed = parseFor('recall', '  > rust ');
		expect(parsed).toEqual({ mode: 'all', text: '> rust' });
		expect(kindsIn('recall', parsed)).toEqual(['prompts']);
	});

	it('leaves the full palette as it was: prefixes narrow, every kind is searched', () => {
		expect(parseFor('full', '@ s-1')).toEqual(parseQuery('@ s-1'));
		expect(kindsIn('full', parseFor('full', 'rust'))).toEqual(['prompts', 'sessions', 'projects']);
		expect(kindsIn('full', parseFor('full', '#x'))).toEqual(['projects']);
	});
});

describe('what recall lists', () => {
	const view = buildView(parseQuery('rust'), recent, true, []);

	it('keeps the prompts and drops the other kinds', () => {
		expect(view.groups.map((g) => g.kind)).toContain('session');
		const only = restrict('recall', view);
		expect(only.groups.map((g) => g.kind)).toEqual(['prompt']);
		expect(rowsOf(only.groups).every((r) => r.item.kind === 'prompt')).toBe(true);
	});

	it('offers no way out to the Prompts page: a search without a match says so', () => {
		const none = { prompts: [], sessions: [], projects: [] };
		const nothing = buildView(parseQuery('zzz'), none, true, []);
		expect(nothing.noMatch).toBe(true);
		expect(restrict('recall', nothing)).toEqual({ groups: [], noMatch: true });
	});

	it('shows the full palette untouched', () => {
		expect(restrict('full', view)).toBe(view);
	});
});

describe('what ↵ does in recall', () => {
	it('copies: ↵ runs the copy action, and the footer lists it on ↵', () => {
		expect(actionKeyIn('recall', 'enter')).toBe('mod+enter');
		expect(actionKeyIn('recall', 'mod+enter')).toBe('mod+enter');
		expect(actionKeyIn('full', 'enter')).toBe('enter');
		expect(actionsIn('recall', 'prompt').map((a) => [a.key, a.label])).toEqual([['enter', 'copy']]);
		expect(actionsIn('full', 'prompt')).toEqual(actionsFor('prompt'));
	});

	it('copies the prompt the row is for when ↵ is pressed, and opens nothing', async () => {
		const copy = vi.fn((text: unknown) => Promise.resolve(text !== undefined));
		const ctx: ActionContext = {
			close: vi.fn(),
			navigate: vi.fn(),
			copy,
			promptText: () => Promise.resolve('the full text'),
			fail: vi.fn()
		};
		await perform({ kind: 'prompt', id: 7 }, actionKeyIn('recall', 'enter'), ctx);
		expect(copy).toHaveBeenCalledTimes(1);
		expect(ctx.navigate).not.toHaveBeenCalled();
		await expect(copy.mock.calls[0][0]).resolves.toBe('the full text');
	});
});

describe('leaving after a copy', () => {
	it('starts the copy at once and leaves once the text is on the clipboard', async () => {
		const order: string[] = [];
		let release: (ok: boolean) => void = () => {};
		const copy = vi.fn(() => {
			order.push('copy started');
			return new Promise<boolean>((resolve) => (release = resolve));
		});
		const done = vi.fn(() => void order.push('done'));
		const copying = copyThenDone(copy, done)('text');
		expect(order).toEqual(['copy started']);
		release(true);
		await expect(copying).resolves.toBe(true);
		expect(order).toEqual(['copy started', 'done']);
	});

	it('stays open when the copy fails, so its message can be read', async () => {
		const done = vi.fn();
		await expect(copyThenDone(() => Promise.resolve(false), done)('text')).resolves.toBe(false);
		expect(done).not.toHaveBeenCalled();
	});
});

describe('what the dialog says', () => {
	it('names the job in each mode', () => {
		expect(wordsFor('full').dialog).toBe('Find or run a command');
		expect(wordsFor('recall').dialog).toBe('Find a prompt to copy');
		expect(wordsFor('recall').placeholder).not.toContain('>');
	});
});
