import { describe, expect, it, vi } from 'vitest';
import type { LeakDecision, LeakSaved } from '#lib/api/types.ts';
import { decideWithApi, LeakDecider } from '#lib/leaks/decide.svelte.ts';
import { HideValues } from '#lib/leaks/hide.svelte.ts';
import type { LeakAction } from '#lib/shared/usage-actions.ts';
import { leakOut, leaks } from '#lib/testing/builders.ts';

const calls = vi.hoisted((): string[] => []);
vi.mock('#lib/api/client.ts', () => ({
	api: {
		leaks: (query: object) => {
			calls.push(`GET leaks ${JSON.stringify(query)}`);
			return Promise.resolve({
				open: [{ id: 'f1', kind: 'JWT', masked: 'eyJ…', place: 'prompt', prompt: 1, at: '' }],
				hidden: [],
				dismissed: [],
				checked: { prompts: 1, kinds: 28 }
			});
		},
		setLeak: (id: string, state: string | null) => {
			calls.push(`PUT ${id} ${state}`);
			return Promise.resolve({ id, state, updated: 0 });
		}
	}
}));
vi.mock('#lib/shared/usage.ts', () => ({
	track: (action: string) => calls.push(`count ${action}`)
}));

/** A decider over finding f1 whose saves, counts and reloads are recorded. */
function setup(fail = false) {
	const log: string[] = [];
	const decider = new LeakDecider('f1', {
		set: (id, state: LeakDecision | null) => {
			if (fail) return Promise.reject(new Error('daemon down'));
			log.push(`PUT ${id} ${state}`);
			const saved: LeakSaved = { id, state, updated: 0 };
			return Promise.resolve(saved);
		},
		hideFlow: (findings) =>
			new HideValues({
				findings,
				load: () => Promise.resolve(leaks({ open: [leakOut({ id: 'f1' })] })),
				hide: (id) => {
					log.push(`PUT ${id} hidden`);
					return Promise.resolve({ id, state: 'hidden', updated: 2 });
				}
			}),
		count: (action: LeakAction) => log.push(`count ${action}`),
		onchanged: () => log.push('reload')
	});
	return { decider, log };
}

describe('Hide', () => {
	it('asks first, then PUTs hidden once confirmed, counts leak.hide and reloads', async () => {
		const { decider, log } = setup();
		decider.ask();
		const flow = decider.asking;
		if (!flow) throw new Error('Hide did not ask');
		await flow.load();
		expect(flow.phase).toBe('confirm');
		expect(log).toEqual([]);
		decider.hidden(await flow.confirm());
		expect(log).toEqual(['PUT f1 hidden', 'count leak.hide', 'reload']);
		expect(decider.asking).toBeNull();
	});

	it('saves and counts nothing when cancelled', () => {
		const { decider, log } = setup();
		decider.ask();
		decider.cancel();
		decider.hidden(0);
		expect([decider.asking, log]).toEqual([null, []]);
	});
});

describe('Not sensitive', () => {
	it('PUTs not_sensitive at once, counts leak.dismiss and reloads', async () => {
		const { decider, log } = setup();
		expect(await decider.dismiss()).toBe(true);
		expect(log).toEqual(['PUT f1 not_sensitive', 'count leak.dismiss', 'reload']);
	});

	it('says why it failed, counting nothing', async () => {
		const { decider, log } = setup(true);
		expect(await decider.dismiss()).toBe(false);
		expect(decider.task.error).toContain('daemon down');
		expect(log).toEqual([]);
	});
});

describe('Undo', () => {
	it('opens a dismissed finding again (state null), counts leak.undo and reloads', async () => {
		const { decider, log } = setup();
		await decider.undo();
		expect(log).toEqual(['PUT f1 null', 'count leak.undo', 'reload']);
	});
});

describe('decideWithApi', () => {
	it('reads the board of the query for Hide, and PUTs over the daemon', async () => {
		calls.length = 0;
		const decider = decideWithApi('f1', { query: { session: 's-1' } });
		decider.ask();
		await decider.asking?.load();
		decider.hidden((await decider.asking?.confirm()) ?? 0);
		await decider.dismiss();
		expect(calls).toEqual([
			'GET leaks {"session":"s-1"}',
			'PUT f1 hidden',
			'count leak.hide',
			'PUT f1 not_sensitive',
			'count leak.dismiss'
		]);
	});
});
