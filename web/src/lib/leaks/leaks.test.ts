import { describe, expect, it } from 'vitest';
import {
	checkedText,
	groupByKind,
	hiddenOnText,
	kindTitle,
	leaksOfPrompt,
	leaksState,
	openByPrompt,
	placeText,
	sessionQuery
} from '#lib/leaks/leaks.ts';
import { leakOut, leaks } from '#lib/testing/builders.ts';

const NOW = new Date('2026-11-18T12:00:00Z');

describe('the words', () => {
	it('say where a value sits, for every place', () => {
		expect(placeText('prompt')).toBe('in your prompt');
		expect(placeText('answer')).toBe('in the answer');
		expect(placeText('tool_input')).toBe('in a tool’s input');
		expect(placeText('tool_output')).toBe('in a tool’s output');
	});

	it('name a kind as a heading', () => {
		expect(kindTitle('email address')).toBe('Email address');
		expect(kindTitle('AWS access key')).toBe('AWS access key');
	});

	it('say the day a value was hidden, else the day it was found', () => {
		const hidden = leakOut({ decided: 'hidden', decided_at: '2026-11-16T09:00:00Z' });
		expect(hiddenOnText(hidden, NOW)).toBe('Hidden on 16 Nov');
		expect(hiddenOnText(leakOut({ at: '2026-11-05T10:00:00Z' }), NOW)).toBe('Hidden on 5 Nov');
	});

	it('say what was checked', () => {
		expect(checkedText({ prompts: 1204, kinds: 28 })).toBe(
			'1,204 prompts checked for 28 kinds of secret and personal data'
		);
		expect(checkedText({ prompts: 1, kinds: 28 })).toBe(
			'1 prompt checked for 28 kinds of secret and personal data'
		);
	});
});

describe('leaksState', () => {
	it('is All clear, saying what was checked, without open findings', () => {
		expect(leaksState(leaks({ checked: { prompts: 1204, kinds: 28 } }))).toEqual({
			kind: 'empty',
			title: 'All clear',
			message: '1,204 prompts checked for 28 kinds of secret and personal data.'
		});
	});

	it('shows the findings when there are some, decided ones aside', () => {
		expect(leaksState(leaks({ open: [leakOut()] }))).toEqual({ kind: 'ready' });
		expect(leaksState(leaks({ hidden: [leakOut()] })).kind).toBe('empty');
	});
});

describe('groupByKind', () => {
	it('groups the findings by kind, kinds in the order they first appear', () => {
		const groups = groupByKind([
			leakOut({ id: 'a', kind: 'JWT' }),
			leakOut({ id: 'b', kind: 'email address' }),
			leakOut({ id: 'c', kind: 'JWT' })
		]);
		expect(groups.map((g) => [g.kind, g.findings.map((f) => f.id)])).toEqual([
			['JWT', ['a', 'c']],
			['email address', ['b']]
		]);
	});
});

describe('the findings of a prompt', () => {
	const board = leaks({
		open: [leakOut({ id: 'a', prompt: 1 }), leakOut({ id: 'b', prompt: 2 })],
		hidden: [leakOut({ id: 'h', prompt: 1 }), leakOut({ id: 'x', prompt: 3 })],
		dismissed: [leakOut({ id: 'd', prompt: 1 })]
	});

	it('are its open and hidden ones; dismissed ones show nowhere', () => {
		const mine = leaksOfPrompt(board, 1);
		expect(mine.open.map((l) => l.id)).toEqual(['a']);
		expect(mine.hidden.map((l) => l.id)).toEqual(['h']);
	});

	it('are none while the leaks are not there', () => {
		expect(leaksOfPrompt(null, 1)).toEqual({ open: [], hidden: [] });
	});

	it('are counted per prompt, open ones only', () => {
		const counts = openByPrompt(
			leaks({ open: [...board.open, leakOut({ id: 'c', prompt: 2 })], hidden: board.hidden })
		);
		expect([...counts]).toEqual([
			[1, 1],
			[2, 2]
		]);
		expect(openByPrompt(undefined).size).toBe(0);
	});
});

describe('sessionQuery', () => {
	it('asks for the session, or for everything without one', () => {
		expect(sessionQuery('s-1')).toEqual({ session: 's-1' });
		expect(sessionQuery(null)).toEqual({});
	});
});
