import { afterEach, describe, expect, it } from 'vitest';
import {
	actionsFor,
	perform,
	registerAction,
	type ActionContext,
	type Item
} from '#lib/palette/actions.ts';

function context(overrides: Partial<ActionContext> = {}) {
	const log: string[] = [];
	const ctx: ActionContext = {
		navigate: (href) => void log.push(`go ${href}`),
		copy: async (text) => log.push(`copy ${await text}`) > 0,
		close: () => void log.push('close'),
		promptText: (id) => Promise.resolve(`text of ${id}`),
		fail: (error) => void log.push(`fail ${String(error)}`),
		...overrides
	};
	return { ctx, log };
}

const removers: (() => void)[] = [];
afterEach(() => {
	for (const remove of removers.splice(0)) remove();
});

describe('the actions of each kind', () => {
	it('opens a prompt, a session and a project on Enter, through links.ts', async () => {
		const { ctx, log } = context();
		await perform({ kind: 'prompt', id: 7 }, 'enter', ctx);
		await perform({ kind: 'session', id: 's-1' }, 'enter', ctx);
		await perform({ kind: 'project', id: 'gp' }, 'enter', ctx);
		expect(log).toEqual(['go /prompts?id=7', 'go /sessions/s-1', 'go /projects/gp']);
	});

	it('copies the full text of a prompt on ⌘↵', async () => {
		const { ctx, log } = context();
		expect(await perform({ kind: 'prompt', id: 7 }, 'mod+enter', ctx)).toBe(true);
		expect(log).toEqual(['copy text of 7']);
	});

	it('closes the dialog, then runs a command', async () => {
		const { ctx, log } = context();
		const command = { id: 'c', title: 'C', keywords: [], run: () => void log.push('ran') };
		await perform({ kind: 'command', command }, 'enter', ctx);
		expect(log).toEqual(['close', 'ran']);
	});

	it('does nothing for a key its kind has no action on', async () => {
		const { ctx, log } = context();
		expect(await perform({ kind: 'session', id: 's' }, 'mod+enter', ctx)).toBe(false);
		expect(log).toEqual([]);
	});

	it('reports an action that fails', async () => {
		const { ctx, log } = context({ promptText: () => Promise.reject(new Error('gone')) });
		await perform({ kind: 'prompt', id: 1 }, 'mod+enter', ctx);
		expect(log).toEqual(['fail Error: gone']);
	});

	it('lists a kind’s actions in key order, so the footer reads ↵ then ⌘↵', () => {
		expect(actionsFor('prompt').map((a) => [a.key, a.label])).toEqual([
			['enter', 'open'],
			['mod+enter', 'copy']
		]);
	});
});

describe('adding an action to a kind (the extension point)', () => {
	it('runs a new key on that kind only, after the built-in ones', async () => {
		const { ctx, log } = context();
		removers.push(
			registerAction('session', {
				key: 'alt+enter',
				label: 'copy resume',
				run: (item, c) => c.copy(`resume ${item.id}`)
			})
		);
		expect(actionsFor('session').map((a) => a.key)).toEqual(['enter', 'alt+enter']);
		expect(actionsFor('project').map((a) => a.key)).toEqual(['enter']);
		await perform({ kind: 'session', id: 's-9' }, 'alt+enter', ctx);
		expect(log).toEqual(['copy resume s-9']);
	});

	it('replaces the action on a key already taken, and can be taken back', async () => {
		const { ctx, log } = context();
		const remove = registerAction('project', {
			key: 'enter',
			label: 'peek',
			run: (i, c) => c.close()
		});
		expect(actionsFor('project').map((a) => a.label)).toEqual(['peek']);
		await perform({ kind: 'project', id: 'p' }, 'enter', ctx);
		expect(log).toEqual(['close']);
		remove();
		expect(actionsFor('project')).toEqual([]);
	});

	it('never hands an action an item of another kind', () => {
		const seen: Item[] = [];
		removers.push(
			registerAction('session', { key: 'alt+enter', label: 'x', run: (i) => seen.push(i) })
		);
		const stored = actionsFor('session').find((a) => a.key === 'alt+enter');
		stored?.run({ kind: 'project', id: 'p' }, context().ctx);
		expect(seen).toEqual([]);
	});
});
