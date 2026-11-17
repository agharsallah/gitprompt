import { beforeEach, describe, expect, it, vi } from 'vitest';
import { perform, type ActionContext } from '#lib/palette/actions.ts';
import { copyText } from '#lib/palette/copy.ts';
import { recent } from '#lib/palette/fixtures.ts';
import { intentOf, isShortcut, moveActive, type KeyLike } from '#lib/palette/keys.ts';
import { PaletteState } from '#lib/palette/open.svelte.ts';
import { parseQuery } from '#lib/palette/query.ts';
import { buildView, rowsOf } from '#lib/palette/rows.ts';

const tracked = vi.hoisted(() => [] as string[]);
vi.mock('#lib/shared/usage.ts', () => ({ track: (action: string) => tracked.push(action) }));

const press = (key: string, mods: Partial<KeyLike> = {}): KeyLike => ({
	key,
	metaKey: false,
	ctrlKey: false,
	altKey: false,
	shiftKey: false,
	...mods
});

/** The palette as the dialog drives it, with the keyboard alone and no mouse. */
function session() {
	const palette = new PaletteState();
	const log: string[] = [];
	const full = 'Fix the rust build.\n'.repeat(62);
	const ctx: ActionContext = {
		close: () => palette.hide(),
		navigate: (href) => {
			palette.hide();
			log.push(`go ${href}`);
		},
		copy: (text) => {
			palette.hide();
			return copyText(text, {
				write: async (t) => void log.push(`clipboard ${(await t).length}`),
				say: (message) => void log.push(`toast ${message}`),
				count: () => void tracked.push('palette.copy')
			});
		},
		promptText: () => Promise.resolve(full),
		fail: (error) => void log.push(`fail ${String(error)}`)
	};
	let query = '';
	let active = 0;
	const rows = () => rowsOf(buildView(parseQuery(query), recent, true, []).groups);
	return {
		palette,
		log,
		full,
		type: (text: string) => {
			query = text;
			active = 0;
		},
		key: async (event: KeyLike) => {
			const intent = intentOf(event);
			if (intent?.type === 'move') active = moveActive(active, intent.delta, rows().length);
			if (intent?.type === 'act') await perform(rows()[active].item, intent.key, ctx);
		},
		highlighted: () => rows()[active]?.key
	};
}

beforeEach(() => (tracked.length = 0));

describe('finding and copying a prompt without the mouse', () => {
	it('opens, types, moves, copies with ⌘↵, and is closed with a toast', async () => {
		const s = session();
		expect(isShortcut(press('k', { metaKey: true }))).toBe(true);
		s.palette.toggle();
		expect(s.palette.open).toBe(true);
		expect(s.palette.loaded).toBe(true);

		s.type('rust');
		expect(s.highlighted()).toBe('prompt:7');
		await s.key(press('ArrowDown'));
		expect(s.highlighted()).toBe('session:s-1');
		await s.key(press('ArrowUp'));
		expect(s.highlighted()).toBe('prompt:7');

		await s.key(press('Enter', { metaKey: true }));

		expect(s.log).toEqual([`clipboard ${s.full.length}`, 'toast Copied · 1,240 characters']);
		expect(s.palette.open).toBe(false);
		expect(tracked).toEqual(['palette.open', 'palette.copy']);
	});

	it('opens a prompt on Enter and closes', async () => {
		const s = session();
		s.palette.show();
		s.type('rust');
		await s.key(press('Enter'));
		expect(s.log).toEqual(['go /prompts?id=7']);
		expect(s.palette.open).toBe(false);
	});

	it('opens your last session first when nothing is typed', async () => {
		const s = session();
		s.palette.show();
		expect(s.highlighted()).toBe('session:s-1');
		await s.key(press('Enter'));
		expect(s.log).toEqual(['go /sessions/s-1']);
	});

	it('closes on the shortcut again, and counts each opening', () => {
		const s = session();
		s.palette.toggle();
		s.palette.toggle();
		s.palette.toggle();
		expect(s.palette.open).toBe(true);
		expect(s.palette.opened).toBe(2);
		expect(tracked).toEqual(['palette.open', 'palette.open']);
	});
});
