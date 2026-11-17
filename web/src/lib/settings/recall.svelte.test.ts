import { describe, expect, it, vi } from 'vitest';
import type { ShortcutResult } from '#lib/platform/types.ts';
import { RecallShortcut } from '#lib/settings/recall.svelte.ts';

/** The store and the app, recording what they are asked, in order. */
function setup(register: (shortcut: string) => ShortcutResult, saved = true) {
	const log: string[] = [];
	const shortcut = new RecallShortcut(
		{
			value: () => 'CmdOrCtrl+Alt+P',
			save: (text) => {
				log.push(`save ${text}`);
				return Promise.resolve(saved);
			}
		},
		{
			problem: () => Promise.resolve('taken at start'),
			register: (text) => {
				log.push(`register ${text}`);
				return Promise.resolve(register(text));
			}
		}
	);
	return { shortcut, log };
}

describe('choosing the recall shortcut', () => {
	it('registers it, then saves it, and clears any problem', async () => {
		const { shortcut, log } = setup(() => ({ kind: 'registered' }));
		await shortcut.load();
		expect(shortcut.problem).toBe('taken at start');
		await shortcut.choose('CmdOrCtrl+Alt+K');
		expect(log).toEqual(['register CmdOrCtrl+Alt+K', 'save CmdOrCtrl+Alt+K']);
		expect(shortcut.problem).toBeNull();
	});

	it('shows a shortcut another app holds and saves nothing', async () => {
		const { shortcut, log } = setup(() => ({ kind: 'refused', reason: 'Ctrl+K is not available' }));
		shortcut.recording = true;
		await shortcut.choose('Ctrl+K');
		expect(shortcut.problem).toBe('Ctrl+K is not available');
		expect(shortcut.recording).toBe(false);
		expect(log).toEqual(['register Ctrl+K']);
	});

	it('takes the old shortcut back when saving the new one fails', async () => {
		const { shortcut, log } = setup(() => ({ kind: 'registered' }), false);
		await shortcut.choose('CmdOrCtrl+Alt+K');
		expect(log).toEqual([
			'register CmdOrCtrl+Alt+K',
			'save CmdOrCtrl+Alt+K',
			'register CmdOrCtrl+Alt+P'
		]);
	});

	it('has no problem to report when the app cannot say', async () => {
		const shortcut = new RecallShortcut(
			{ value: () => '', save: vi.fn() },
			{ problem: () => Promise.reject(new Error('no shell')), register: vi.fn() }
		);
		await shortcut.load();
		expect(shortcut.problem).toBeNull();
	});
});
