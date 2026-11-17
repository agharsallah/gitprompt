/** Test data: what `GET /api/find` answers, by kind. */
import type { FindHit, FindResults } from '#lib/api/types.ts';

export const hit = (overrides: Partial<FindHit> & Pick<FindHit, 'target'>): FindHit => ({
	title: null,
	snippet: [{ text: 'a prompt', hit: false }],
	at: '2026-11-16T10:00:00Z',
	outcome: null,
	...overrides
});

export const results = (parts: Partial<FindResults> = {}): FindResults => ({
	prompts: [],
	sessions: [],
	projects: [],
	...parts
});

/** One of each kind, as an empty search (the most recent) would answer. */
export const recent: FindResults = results({
	prompts: [
		hit({
			target: { kind: 'prompt', id: 7 },
			title: 'Block flaky timing',
			snippet: [
				{ text: 'Fix the ', hit: false },
				{ text: 'rust', hit: true },
				{ text: ' build', hit: false }
			]
		})
	],
	sessions: [hit({ target: { kind: 'session', id: 's-1' }, title: 'Port the store' })],
	projects: [hit({ target: { kind: 'project', id: 'gitprompt' }, title: 'gitprompt' })]
});
