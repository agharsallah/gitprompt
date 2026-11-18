import { describe, expect, it } from 'vitest';
import { kindsFor, parseQuery } from '#lib/palette/query.ts';

describe('parseQuery', () => {
	it('searches everything without a prefix', () => {
		expect(parseQuery('  fix test ')).toEqual({ mode: 'all', text: 'fix test' });
		expect(parseQuery('')).toEqual({ mode: 'all', text: '' });
	});

	it.each([
		['> exp', 'commands', 'exp'],
		['#git', 'projects', 'git'],
		['@ port the', 'sessions', 'port the'],
		['>', 'commands', '']
	])('reads %j as the %s mode', (raw, mode, text) => {
		expect(parseQuery(raw)).toEqual({ mode, text });
	});

	it('leaves a prefix in the middle alone', () => {
		expect(parseQuery('a > b')).toEqual({ mode: 'all', text: 'a > b' });
	});

	it('asks the API for the kinds of the mode, and for none for commands', () => {
		expect(kindsFor('all')).toEqual(['prompts', 'sessions', 'projects', 'commits']);
		expect(kindsFor('projects')).toEqual(['projects']);
		expect(kindsFor('sessions')).toEqual(['sessions']);
		expect(kindsFor('commands')).toEqual([]);
	});
});
