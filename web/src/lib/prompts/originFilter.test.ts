import { describe, expect, it } from 'vitest';
import {
	ALL_ORIGINS,
	DEFAULT_ORIGINS,
	isAll,
	parseOrigins,
	readOrigins,
	writeOrigins
} from '#lib/prompts/originFilter.svelte.ts';

function memory(initial?: string) {
	const items = new Map<string, string>(
		initial === undefined ? [] : [['gitprompt-origins', initial]]
	);
	return {
		getItem: (k: string) => items.get(k) ?? null,
		setItem: (k: string, v: string) => void items.set(k, v)
	};
}

const broken = {
	getItem: () => {
		throw new Error('blocked');
	},
	setItem: () => {
		throw new Error('blocked');
	}
};

describe('origin filter', () => {
	it('defaults to what you typed and the slash commands you ran', () => {
		expect(DEFAULT_ORIGINS).toEqual(['typed', 'slash_command']);
		expect(readOrigins(memory())).toEqual(DEFAULT_ORIGINS);
		expect(readOrigins(undefined)).toEqual(DEFAULT_ORIGINS);
	});

	it('remembers the choice, including "every origin" (an empty list)', () => {
		const storage = memory();
		writeOrigins(['task_notification', 'typed'], storage);
		expect(readOrigins(storage)).toEqual(['typed', 'task_notification']);
		writeOrigins([], storage);
		expect(readOrigins(storage)).toEqual([]);
	});

	it('ignores unknown, duplicate and corrupt values', () => {
		expect(parseOrigins(['nope', 'subagent', 'subagent'])).toEqual(['subagent']);
		expect(readOrigins(memory('{"not": "a list"}'))).toEqual(DEFAULT_ORIGINS);
		expect(readOrigins(memory('not json'))).toEqual(DEFAULT_ORIGINS);
	});

	it('works without storage', () => {
		expect(readOrigins(broken)).toEqual(DEFAULT_ORIGINS);
		expect(() => writeOrigins(['typed'], broken)).not.toThrow();
	});

	it('treats none and all as no filter', () => {
		expect(isAll([])).toBe(true);
		expect(isAll(ALL_ORIGINS)).toBe(true);
		expect(isAll(DEFAULT_ORIGINS)).toBe(false);
	});
});
