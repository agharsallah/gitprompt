import { describe, expect, it, vi } from 'vitest';
import type { Leaks, LeakSaved } from '#lib/api/types.ts';
import {
	HideValues,
	hiddenText,
	hideWithApi,
	pickFindings,
	PLACE_WORDS,
	type HideOptions
} from '#lib/leaks/hide.svelte.ts';
import { leakOut } from '#lib/testing/builders.ts';

const calls = vi.hoisted((): string[] => []);
vi.mock('#lib/api/client.ts', () => ({
	api: {
		leaks: (query: object) => {
			calls.push(`GET leaks ${JSON.stringify(query)}`);
			return Promise.resolve(board(['a', 'b']));
		},
		setLeak: (id: string, state: string | null) => {
			calls.push(`PUT ${id} ${state}`);
			return Promise.resolve({ id, state, updated: 1 });
		}
	}
}));

const board = (ids: string[]): Leaks => ({
	open: ids.map((id) => leakOut({ id, masked: `${id}•••` })),
	hidden: [],
	dismissed: [],
	checked: { prompts: 10, kinds: 26 }
});

/** A flow over findings `ids` whose saves are recorded; `failOn` makes that one fail. */
function setup(ids: string[], overrides: Partial<HideOptions> = {}, failOn?: string) {
	const saved: string[] = [];
	const flow = new HideValues({
		findings: ids,
		load: () => Promise.resolve(board(['a', 'b', 'c'])),
		hide: (id) => {
			if (id === failOn) return Promise.reject(new Error('value gone'));
			saved.push(id);
			const answer: LeakSaved = { id, state: 'hidden', updated: 1 };
			return Promise.resolve(answer);
		},
		...overrides
	});
	return { flow, saved };
}

describe('pickFindings', () => {
	it('keeps the open findings asked for, in the order asked', () => {
		const picked = pickFindings(board(['a', 'b', 'c']), ['c', 'x', 'a']);
		expect(picked.map((l) => l.id)).toEqual(['c', 'a']);
	});
});

describe('the words', () => {
	it('say where a value sits and how many were hidden', () => {
		expect(PLACE_WORDS.tool_output).toBe('a tool’s output');
		expect(PLACE_WORDS.answer).toBe('the answer');
		expect(hiddenText(2)).toBe('Hidden 2 values');
		expect(hiddenText(1)).toBe('Hidden 1 value');
	});
});

describe('HideValues', () => {
	it('lists the values and hides nothing before the confirm', async () => {
		const { flow, saved } = setup(['b', 'a']);
		await flow.load();
		expect(flow.phase).toBe('confirm');
		expect(flow.found.map((l) => l.masked)).toEqual(['b•••', 'a•••']);
		expect(saved).toEqual([]);
	});

	it('sends hidden for each finding once confirmed', async () => {
		const { flow, saved } = setup(['b', 'a']);
		await flow.load();
		expect(await flow.confirm()).toBe(2);
		expect([flow.phase, saved]).toEqual(['done', ['b', 'a']]);
	});

	it('saves nothing when confirming before the list is shown', async () => {
		const { flow, saved } = setup(['a']);
		expect(await flow.confirm()).toBe(0);
		expect(saved).toEqual([]);
	});

	it('says when the findings are no longer open', async () => {
		const { flow } = setup(['zzz']);
		await flow.load();
		expect(flow.phase).toBe('error');
		expect(flow.error).toContain('no longer open');
	});

	it('says why loading failed', async () => {
		const { flow } = setup(['a'], { load: () => Promise.reject(new Error('offline')) });
		await flow.load();
		expect(flow.error).toContain('offline');
	});

	it('stops at the first value that fails, keeping the count of those hidden', async () => {
		const { flow, saved } = setup(['a', 'b', 'c'], {}, 'b');
		await flow.load();
		expect(await flow.confirm()).toBe(1);
		expect([flow.phase, saved, flow.hidden]).toEqual(['error', ['a'], 1]);
		expect(flow.error).toContain('b•••');
	});
});

describe('hideWithApi', () => {
	it('reads the board of the range, then PUTs hidden per finding only after the confirm', async () => {
		calls.length = 0;
		const flow = hideWithApi(['b', 'a'], { since: '2026-11-01' });
		await flow.load();
		expect(calls).toEqual(['GET leaks {"since":"2026-11-01"}']);
		await flow.confirm();
		expect(calls.slice(1)).toEqual(['PUT b hidden', 'PUT a hidden']);
	});
});
