import { describe, expect, it } from 'vitest';
import { allowSummary, allowText, parseAllow, sameAllow } from '#lib/settings/allow.ts';

describe('the values never flagged', () => {
	it('are read from commas and new lines, trimmed, each kept once', () => {
		expect(parseAllow(' me@work.example,\ntest@example.com, ,me@work.example\n')).toEqual([
			'me@work.example',
			'test@example.com'
		]);
		expect(parseAllow('')).toEqual([]);
	});

	it('are shown as one list', () => {
		expect(allowText(['a@x.io', 'b@x.io'])).toBe('a@x.io, b@x.io');
		expect(allowSummary([])).toBe('None');
		expect(allowSummary(['a@x.io'])).toBe('a@x.io');
	});

	it('need no save when the text says the stored list', () => {
		expect(sameAllow('a@x.io,b@x.io ', ['a@x.io', 'b@x.io'])).toBe(true);
		expect(sameAllow('a@x.io', ['a@x.io', 'b@x.io'])).toBe(false);
	});
});
