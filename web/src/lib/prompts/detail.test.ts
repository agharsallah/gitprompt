import { describe, expect, it } from 'vitest';
import {
	capturedFields,
	isLong,
	textFacts,
	turnFigures,
	unmappedPayload
} from '#lib/prompts/detail.ts';
import { factsView } from '#lib/prompts/facts.ts';
import { facts } from '#lib/testing/builders.ts';

const tokens = {
	input: 900,
	output: 100,
	cache_read: 0,
	cache_creation: 0,
	reasoning: 0,
	total: 1000
};

describe('textFacts', () => {
	it('always gives the size, even of an empty text', () =>
		expect(textFacts({})).toEqual([{ kind: 'size', text: '0 words, about 0 tokens' }]));

	it('adds what the text holds, in a fixed order', () =>
		expect(
			textFacts({
				words: 1,
				approx_tokens: 2,
				questions: 2,
				code_blocks: 1,
				urls: ['https://a', 'https://b'],
				at_mentions: ['src/app.ts', 'README.md']
			})
		).toEqual([
			{ kind: 'size', text: '1 word, about 2 tokens' },
			{ kind: 'questions', text: '2 questions' },
			{ kind: 'code', text: '1 code block' },
			{ kind: 'links', text: '2 links' },
			{ kind: 'mentions', text: '@src/app.ts @README.md' }
		]));

	it('leaves out what the text does not hold', () =>
		expect(textFacts({ questions: 0, urls: [], at_mentions: [] }).map((f) => f.kind)).toEqual([
			'size'
		]));
});

describe('turnFigures', () => {
	it('shows tool calls and files, but no time, tokens or cost that were not reported', () => {
		const f = facts();
		expect(turnFigures({}, factsView(f), f)).toEqual([
			{ label: 'Tool calls', value: '0', hint: undefined },
			{ label: 'Files changed', value: '0' }
		]);
	});

	it('flags failed tool calls and notes reasoning tokens', () => {
		const f = facts({ tokens: { ...tokens, reasoning: 2500 }, cost_usd: 0.5 });
		const summary = {
			response_s: 42,
			tool_calls: { Bash: 2, Read: 3 },
			tool_failures: 1,
			files_written: ['a.ts']
		};
		const figures = turnFigures(summary, factsView(f), f);
		expect(figures.map((x) => x.label)).toEqual([
			'Worked for',
			'Tool calls',
			'Files changed',
			'Tokens',
			'Cost'
		]);
		expect(figures[1]).toMatchObject({ value: '5', hint: '1 failed' });
		expect(figures[2].value).toBe('1');
		expect(figures[3]).toMatchObject({ value: '1,000', note: '2,500 reasoning' });
		expect(figures[3].title).toContain('1,000 tokens');
	});

	it('shows a zero-second turn: zero is a time, not a missing one', () => {
		const f = facts();
		expect(turnFigures({ response_s: 0 }, factsView(f), f)[0].label).toBe('Worked for');
	});

	it('has no reasoning note when none was spent', () => {
		const f = facts({ tokens });
		expect(
			turnFigures({}, factsView(f), f).find((x) => x.label === 'Tokens')?.note
		).toBeUndefined();
	});
});

describe('isLong', () => {
	it.each([
		[null, false],
		[undefined, false],
		['', false],
		['short', false],
		['x'.repeat(1400), false],
		['x'.repeat(1401), true],
		[Array(18).fill('line').join('\n'), false],
		[Array(19).fill('line').join('\n'), true]
	])('%j -> %s', (text, long) => expect(isLong(text)).toBe(long));
});

describe('payload fields', () => {
	const fields = { session_id: 's1', prompt: 'Fix it', cwd: '/work' };

	it('captures every mapped field but the prompt text', () =>
		expect(capturedFields(fields)).toEqual({ session_id: 's1', cwd: '/work' }));

	it('keeps what the spec did not map, so new metadata shows up', () =>
		expect(
			unmappedPayload({ session_id: 's1', prompt: 'Fix it', cwd: '/work', effort: 'high' }, fields)
		).toEqual({ effort: 'high' }));
});
