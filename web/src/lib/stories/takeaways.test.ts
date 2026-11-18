import { describe, expect, it } from 'vitest';
import type { StatsBucket } from '#lib/api/types.ts';
import { boundKey } from '#lib/insights/series.ts';
import {
	activityTakeaway,
	agenticTakeaway,
	costTakeaway,
	mainModel,
	peak,
	starredTakeaway,
	takeaway,
	typesTakeaway,
	typicalSession
} from '#lib/stories/takeaways.ts';
import { bucket, overview, prompt, stats } from '#lib/testing/builders.ts';

const grouped = (groups: StatsBucket[]) =>
	stats({ groups, totals: bucket({ prompts: groups.reduce((n, g) => n + g.prompts, 0) }) });

/** Prompts by ISO weekday (1 = Monday) and hour. */
const week = (cells: [day: number, hour: number, prompts: number][]) =>
	grouped(cells.map(([d, h, prompts]) => bucket({ key: `${d}-${h}`, prompts })));

/** Sessions by the lower bound of their length band, in seconds. */
const lengths = (bandsOf: [bound: number, sessions: number][]) =>
	stats({ groups: bandsOf.map(([b, sessions]) => bucket({ key: boundKey(b), sessions })) });

describe('Activity', () => {
	const busy = week([
		[3, 14, 6],
		[3, 15, 5],
		[4, 16, 5],
		[4, 17, 4],
		[1, 9, 2]
	]);

	it('names the peak days and hours, and the typical session', () => {
		const o = overview({
			week: busy,
			sessionLengths: lengths([
				[300, 2],
				[900, 5],
				[3600, 1]
			])
		});
		expect(activityTakeaway(o)).toBe(
			'Peak: Wed–Thu 14:00–18:00; a typical session lasts 15m to 30m.'
		);
	});

	it('names one day when no neighbour comes close', () => {
		expect(
			peak(
				week([
					[5, 22, 9],
					[4, 1, 2],
					[6, 0, 1]
				])
			)
		).toBe('peak: Fri 19:00–23:00');
	});

	it('says nothing of a peak with too few prompts, or of sessions with none', () => {
		expect(peak(week([[3, 14, 3]]))).toBeNull();
		expect(typicalSession(lengths([]))).toBeNull();
		expect(activityTakeaway(overview())).toBeNull();
	});

	it('says the length band of the median session', () => {
		expect(typicalSession(lengths([[0, 3]]))).toBe('a typical session lasts under 1m');
		expect(typicalSession(lengths([[14400, 1]]))).toBe('a typical session lasts 4h or more');
	});
});

describe('Prompt types', () => {
	const origins = grouped([
		bucket({ key: 'typed', prompts: 57 }),
		bucket({ key: 'task_notification', prompts: 43 })
	]);

	it('says your share, what was sent again and the command run most', () => {
		const o = overview({
			origins,
			activity: stats({ totals: bucket({ prompts: 100, distinct_texts: 81 }) }),
			telemetry: {
				sessions: 3,
				metrics: {},
				commands: [
					{ command: '/clear', source: 'builtin', uses: 4, sessions: 2 },
					{ command: '/effort', source: 'builtin', uses: 19, sessions: 17 }
				]
			}
		});
		expect(typesTakeaway(o)).toBe(
			'57% of prompts came from you; 19 prompts repeated a text sent before; /effort is the command you run most.'
		);
	});

	it('takes the command from the prompts when no tool reports telemetry', () => {
		const o = overview({
			origins,
			commands: grouped([
				bucket({ key: '/review', prompts: 2 }),
				bucket({ key: '/commit', prompts: 5 })
			])
		});
		expect(typesTakeaway(o)).toBe(
			'57% of prompts came from you; /commit is the command you run most.'
		);
	});

	it('says nothing without prompts', () => {
		expect(typesTakeaway(overview())).toBeNull();
	});
});

describe('Models and cost', () => {
	it('names the main model and its share of the spend', () => {
		const models = stats({
			totals: bucket({ cost_usd: 453, api_requests: 6000 }),
			groups: [
				bucket({ key: 'claude-haiku-4-5', cost_usd: 48, api_requests: 1100 }),
				bucket({ key: 'claude-opus-4-1', cost_usd: 405, api_requests: 4900 })
			]
		});
		expect(costTakeaway(overview({ models }))).toMatch(/^Opus 4\.1 took 89% of the \$453 spent\.$/);
	});

	it('falls back to requests, then to prompts, when nothing is priced', () => {
		const requests = stats({
			totals: bucket({ api_requests: 10 }),
			groups: [bucket({ key: 'gpt-5', api_requests: 8 }), bucket({ key: 'o3', api_requests: 2 })]
		});
		expect(mainModel(requests)).toMatch(/answered 80% of the model requests$/);
		const prompts = stats({
			totals: bucket({ prompts: 4 }),
			groups: [bucket({ key: 'gpt-5', prompts: 3 })]
		});
		expect(mainModel(prompts)).toMatch(/answered 75% of the prompts$/);
	});

	it('says nothing when no model is named', () => {
		expect(costTakeaway(overview())).toBeNull();
		expect(
			mainModel(stats({ totals: bucket({ cost_usd: 3 }), groups: [bucket({ cost_usd: 3 })] }))
		).toBeNull();
	});
});

describe('Agentic work', () => {
	it('says the tool calls, the kind most were, and the share of ended turns that were done', () => {
		const o = overview({
			toolKinds: grouped([
				bucket({ key: 'read', tool_calls: 47 }),
				bucket({ key: 'shell', tool_calls: 30 }),
				bucket({ key: 'write', tool_calls: 23 })
			]),
			outcomes: grouped([
				bucket({ key: 'done', prompts: 41 }),
				bucket({ key: 'failed', prompts: 4 }),
				bucket({ key: 'interrupted', prompts: 5 }),
				bucket({ key: 'open', prompts: 30 })
			])
		});
		expect(agenticTakeaway(o)).toBe(
			'The agent made 100 tool calls, 47% read & search; 82% of the turns that ended were done.'
		);
	});

	it('says what it has, and nothing without either', () => {
		const ended = overview({ outcomes: grouped([bucket({ key: 'done', prompts: 2 })]) });
		expect(agenticTakeaway(ended)).toBe('100% of the turns that ended were done.');
		expect(agenticTakeaway(overview())).toBeNull();
	});
});

describe('Starred', () => {
	it('counts the starred prompts, or says how many it shows at the limit', () => {
		expect(starredTakeaway(3, 5)).toBe('3 prompts starred in this window.');
		expect(starredTakeaway(1, 5)).toBe('1 prompt starred in this window.');
		expect(starredTakeaway(5, 5)).toBe(
			'Your 5 latest starred prompts; All starred lists every one.'
		);
	});

	it('says nothing when none is starred', () => expect(starredTakeaway(0, 5)).toBeNull());
});

describe('takeaway', () => {
	it('picks the tab’s own sentence', () => {
		const o = overview({ starred: [prompt(), prompt()] });
		expect(takeaway('starred', o, 5)).toBe('2 prompts starred in this window.');
		for (const tab of ['activity', 'types', 'cost', 'agentic'] as const) {
			expect(takeaway(tab, o, 5)).toBeNull();
		}
	});
});
