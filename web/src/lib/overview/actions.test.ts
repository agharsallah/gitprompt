import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { WorthItem } from '#lib/api/types.ts';
import { actionCount, buttonFor, commandFile, copyCommand } from '#lib/overview/actions.ts';
import { worthItem } from '#lib/testing/builders.ts';
import { toasts } from '#lib/ui/toast.svelte.ts';

const tracked = vi.hoisted((): string[] => []);
vi.mock('#lib/shared/usage.ts', () => ({ track: (action: string) => tracked.push(action) }));

/** An item from a source this dashboard has never heard of (the daemon is newer). */
function unknownKind(): WorthItem {
	const item = worthItem({ target: { page: 'session', id: 's1' } });
	return Object.assign(item, { kind: 'stale_branch' });
}

describe('buttonFor', () => {
	it('copies a repeated prompt as a command, naming the file to save it as', () => {
		expect(buttonFor(worthItem())).toEqual({
			type: 'copy',
			label: 'Copy as command',
			text: 'run the tests and fix what fails',
			file: '.claude/commands/run-tests.md'
		});
	});

	it('opens a redone turn in the reader', () => {
		const item = worthItem({
			kind: 'redone_turn',
			action: { type: 'open_turn', prompt_id: 12 },
			target: { page: 'prompt', id: 12 }
		});
		expect(buttonFor(item)).toEqual({
			type: 'open',
			label: 'Open the turn',
			href: '/prompts?id=12'
		});
	});

	it('hides the values of a possible leak', () => {
		const item = worthItem({
			kind: 'possible_leak',
			action: { type: 'hide_values', findings: ['a', 'b'] },
			target: { page: 'leaks' }
		});
		expect(buttonFor(item)).toEqual({ type: 'hide', label: 'Hide values', findings: ['a', 'b'] });
	});

	it('links a kind it does not know to its target', () => {
		expect(buttonFor(unknownKind())).toEqual({
			type: 'link',
			label: 'Have a look',
			href: '/sessions/s1'
		});
	});

	it('links a known kind that carries another action, rather than guess', () => {
		const item = worthItem({ action: { type: 'open_turn', prompt_id: 3 } });
		expect(buttonFor(item).type).toBe('link');
	});
});

describe('copyCommand', () => {
	const written: string[] = [];

	beforeEach(() => {
		tracked.length = 0;
		written.length = 0;
		toasts.items = [];
	});
	afterEach(() => vi.unstubAllGlobals());

	const stubClipboard = (writeText: (text: string) => Promise<void>) =>
		vi.stubGlobal('navigator', { clipboard: { writeText } });

	it('copies the text through the clipboard, counts it and names the file', async () => {
		stubClipboard((text) => {
			written.push(text);
			return Promise.resolve();
		});
		const copy = buttonFor(worthItem());
		if (copy.type !== 'copy') throw new Error('expected a copy');
		expect(await copyCommand('repeated_prompt', copy)).toBe(true);
		expect(written).toEqual(['run the tests and fix what fails']);
		expect(tracked).toEqual(['worth.act.repeated_prompt']);
		expect(toasts.items.map((t) => t.message)).toEqual([
			`Copied · save it as ${commandFile('run-tests')} to use it as a command`
		]);
	});

	it('counts nothing when the clipboard refuses', async () => {
		stubClipboard(() => Promise.reject(new Error('denied')));
		const copy = { type: 'copy', label: '', text: 'x', file: 'f' } as const;
		expect(await copyCommand('repeated_prompt', copy)).toBe(false);
		expect(tracked).toEqual([]);
		expect(toasts.items[0]?.tone).toBe('error');
	});
});

describe('actionCount', () => {
	it('counts per kind', () => expect(actionCount('possible_leak')).toBe('worth.act.possible_leak'));
});
