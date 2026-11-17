import { describe, expect, it } from 'vitest';
import { highlight, wordsOf } from '#lib/palette/highlight.ts';

describe('highlight', () => {
	it('marks every word wherever it appears, in any case', () => {
		expect(highlight('Rust and more rust', ['rust'])).toEqual([
			{ text: 'Rust', hit: true },
			{ text: ' and more ', hit: false },
			{ text: 'rust', hit: true }
		]);
	});

	it('returns the text whole when there is no word or no match', () => {
		expect(highlight('abc', [])).toEqual([{ text: 'abc', hit: false }]);
		expect(highlight('abc', ['x'])).toEqual([{ text: 'abc', hit: false }]);
	});

	it('takes the words literally, not as a pattern', () => {
		expect(highlight('a (b) c.d', ['(b)', 'c.d'])).toEqual([
			{ text: 'a ', hit: false },
			{ text: '(b)', hit: true },
			{ text: ' ', hit: false },
			{ text: 'c.d', hit: true }
		]);
	});

	it('marks the longest word where one word starts another (git, gitprompt)', () => {
		expect(highlight('gitprompt', ['git', 'gitprompt'])).toEqual([
			{ text: 'gitprompt', hit: true }
		]);
		expect(highlight('fixture and fix', ['fix', 'fixture'])).toEqual([
			{ text: 'fixture', hit: true },
			{ text: ' and ', hit: false },
			{ text: 'fix', hit: true }
		]);
	});

	it('splits a query into its words', () => {
		expect(wordsOf('  fix   the test ')).toEqual(['fix', 'the', 'test']);
		expect(wordsOf('')).toEqual([]);
	});
});
