import { describe, expect, it } from 'vitest';
import type { SearchHit } from '#lib/api/types.ts';
import { hitRows } from '#lib/prompts/hits.ts';
import { prompt } from '#lib/testing/builders.ts';

const hit = (id: number): SearchHit => ({
	id,
	received_at: '2026-11-05T10:00:00.000Z',
	session: null,
	title: null,
	project: null,
	branch: null,
	origin: 'typed',
	outcome: 'done',
	cost_usd: 0,
	facts: null,
	snippet: []
});

describe('hitRows', () => {
	it('joins each hit to the text to copy: what was typed, without the injected context', () => {
		const p = prompt({ id: 1, prompt: 'context + words', typed_text: 'words' });
		expect(hitRows([hit(1)], [p])).toEqual([{ hit: hit(1), text: 'words' }]);
	});

	it('keeps the hits’ order and drops those whose prompt a quick filter hid', () => {
		const prompts = [prompt({ id: 3 }), prompt({ id: 1 })];
		expect(hitRows([hit(3), hit(2), hit(1)], prompts).map((r) => r.hit.id)).toEqual([3, 1]);
	});

	it('has no text for a prompt with none captured', () => {
		const p = prompt({ id: 4, prompt: null, typed_text: null });
		expect(hitRows([hit(4)], [p])[0].text).toBeNull();
	});
});
