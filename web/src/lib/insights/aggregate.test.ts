import { describe, expect, it } from 'vitest';
import {
	attention,
	byDay,
	byHour,
	cacheShare,
	craft,
	lanes,
	lengthVsWork,
	median,
	modelMix,
	projects,
	quantile,
	sessions,
	tokensOf,
	toolUse,
	totals,
	weekHours
} from '#lib/insights/aggregate.ts';
import { mergeProject, projectOf } from '#lib/insights/project.ts';
import { prompt } from '#lib/testing/builders.ts';

describe('quantile and median', () => {
	it('interpolates between values', () => {
		expect(median([1, 2, 3, 4])).toBe(2.5);
		expect(quantile([0, 10], 0.9)).toBe(9);
	});

	it('skips missing values and has no answer for none', () => {
		expect(median([undefined, 4, null, 2])).toBe(3);
		expect(median([])).toBeNull();
	});
});

describe('tokens', () => {
	it('reads the four kinds, treating missing ones as zero', () => {
		expect(tokensOf(prompt({ summary: { input_tokens: 5, cache_read_tokens: 90 } }))).toEqual({
			input: 5,
			output: 0,
			cacheRead: 90,
			cacheWrite: 0
		});
	});

	it('measures cache share on the prompt side only', () => {
		expect(cacheShare({ input: 10, output: 1000, cacheRead: 80, cacheWrite: 10 })).toBe(0.8);
		expect(cacheShare({ input: 0, output: 5, cacheRead: 0, cacheWrite: 0 })).toBeNull();
	});
});

describe('totals', () => {
	const now = Date.parse('2026-11-07T12:00:00Z');
	const list = [
		prompt({
			id: 3,
			fields: { session_id: 's1' },
			summary: {
				ended_by: 'Stop',
				cost_usd: 0.5,
				api_requests: 2,
				response_s: 30,
				files_written: ['/a', '/b'],
				tool_calls: { Bash: 2 },
				bash_commands: ['ls', 'pwd']
			}
		}),
		prompt({
			id: 2,
			fields: { session_id: 's1' },
			summary: { ended_by: 'StopFailure', response_s: 10, files_written: ['/a'], tool_failures: 1 }
		}),
		prompt({
			id: 1,
			fields: { session_id: 's2', cwd: '/elsewhere' },
			context: { git_root: undefined },
			summary: {}
		})
	];
	const t = totals(list, now);

	it('counts prompts, sessions and projects', () => {
		expect([t.prompts, t.sessions, t.projects]).toEqual([3, 2, 2]);
	});

	it('sums cost and counts only prompts with telemetry as measured', () => {
		expect(t.cost).toBe(0.5);
		expect(t.withTelemetry).toBe(1);
	});

	it('tallies how turns ended', () => {
		expect(t.outcomes).toEqual({ done: 1, failed: 1, running: 0, open: 1 });
	});

	it('counts each changed file once', () => expect(t.filesWritten).toBe(2));

	it('sums tool work', () => {
		expect([t.toolCalls, t.toolFailures, t.commands]).toEqual([2, 1, 2]);
		expect(t.responseMedian).toBe(20);
	});
});

describe('byDay', () => {
	it('has one entry per local day, empty days included', () => {
		const days = byDay(
			[
				prompt({ received_at: '2026-11-05T09:00:00Z', summary: { cost_usd: 1 } }),
				prompt({ received_at: '2026-11-05T22:30:00Z' }), // 23:30 in Berlin: still the 5th
				prompt({ received_at: '2026-11-07T08:00:00Z', summary: { ended_by: 'StopFailure' } })
			],
			new Date('2026-11-04T12:00:00Z'),
			new Date('2026-11-07T12:00:00Z')
		);
		expect(days.map((d) => [d.key, d.prompts])).toEqual([
			['2026-11-04', 0],
			['2026-11-05', 2],
			['2026-11-06', 0],
			['2026-11-07', 1]
		]);
		expect(days[1].cost).toBe(1);
		expect(days[3].failed).toBe(1);
	});

	it('survives the daylight-saving change', () => {
		const days = byDay([], new Date(2026, 9, 24, 12), new Date(2026, 9, 27, 12));
		expect(days.map((d) => d.key)).toEqual([
			'2026-10-24',
			'2026-10-25',
			'2026-10-26',
			'2026-10-27'
		]);
	});

	it('ignores prompts outside the window', () => {
		const days = byDay(
			[prompt({ received_at: '2026-01-01T10:00:00Z' })],
			new Date(2026, 10, 5),
			new Date(2026, 10, 5)
		);
		expect(days[0].prompts).toBe(0);
	});
});

describe('byHour', () => {
	it('buckets prompts by clock hour', () => {
		const start = new Date('2026-11-05T10:20:00Z');
		const end = new Date('2026-11-05T12:10:00Z');
		const hours = byHour(
			[
				prompt({ received_at: '2026-11-05T10:45:00Z' }),
				prompt({ received_at: '2026-11-05T12:05:00Z' }),
				prompt({ received_at: '2026-11-05T12:30:00Z' })
			],
			start,
			end
		);
		expect(hours.map((h) => h.prompts)).toEqual([1, 0, 1]);
	});
});

describe('weekHours', () => {
	it('counts by local weekday (Monday first) and hour', () => {
		// Thursday 5 November 2026, 10:00 UTC = 11:00 in Berlin.
		const grid = weekHours([prompt({ received_at: '2026-11-05T10:00:00Z' })]);
		expect(grid).toHaveLength(7);
		expect(grid[3][11]).toBe(1);
		expect(grid.flat().reduce((a, b) => a + b)).toBe(1);
		const fromSunday = weekHours([prompt({ received_at: '2026-11-05T10:00:00Z' })], 'sunday');
		expect(fromSunday[4][11]).toBe(1);
	});
});

describe('tools and models', () => {
	const list = [
		prompt({ summary: { tool_calls: { Bash: 3, Read: 1 }, models: { 'claude-opus-5-5': 4 } } }),
		prompt({
			summary: { tool_calls: { Read: 3 }, models: { 'claude-opus-5-5': 1, 'claude-haiku-5-5': 2 } }
		})
	];

	it('ranks tools by calls', () => {
		expect(toolUse(list)).toEqual([
			{ name: 'Read', kind: 'read', count: 4 },
			{ name: 'Bash', kind: 'shell', count: 3 }
		]);
	});

	it('ranks models by requests', () => {
		expect(modelMix(list)).toEqual([
			{ model: 'claude-opus-5-5', requests: 5 },
			{ model: 'claude-haiku-5-5', requests: 2 }
		]);
	});
});

describe('how prompts are written', () => {
	it('relates prompt length to the work that followed', () => {
		const buckets = lengthVsWork([
			prompt({ stats: { words: 3 }, summary: { response_s: 10, tool_calls: { Bash: 1 } } }),
			prompt({ stats: { words: 5 }, summary: { response_s: 20 } }),
			prompt({ stats: { words: 300 }, summary: { response_s: 600, tool_calls: { Edit: 9 } } })
		]);
		expect(buckets[0]).toEqual({ label: '1–5', prompts: 2, medianResponse: 15, medianTools: 0.5 });
		expect(buckets.at(-1)).toMatchObject({
			label: '250+',
			prompts: 1,
			medianResponse: 600,
			medianTools: 9
		});
		expect(buckets[2]).toMatchObject({ prompts: 0, medianResponse: null });
	});

	it('counts the habits of a prompt', () => {
		const c = craft([
			prompt({ stats: { words: 10, code_blocks: 1, questions: 2 } }),
			prompt({ stats: { words: 20, urls: ['https://x'], slash_command: '/review' } }),
			prompt({ stats: { words: 30 }, summary: { slash_command: '/review' } })
		]);
		expect(c).toEqual({
			medianWords: 20,
			withCode: 1,
			withQuestion: 1,
			withLink: 1,
			withMention: 0,
			slashCommands: [{ name: '/review', count: 2 }]
		});
	});
});

describe('attention', () => {
	it('collects what deserves a second look', () => {
		const a = attention([
			prompt({ id: 1, stats: { secrets: ['aws_key'] } }),
			prompt({ id: 2, summary: { ended_by: 'StopFailure' } }),
			prompt({ id: 3, summary: { tool_failures: 2 } }),
			prompt({ id: 4, summary: { tool_rejections: 1 } }),
			prompt({ id: 5 })
		]);
		expect([a.secrets, a.failed, a.toolFailures, a.denied].map((l) => l.map((p) => p.id))).toEqual([
			[1],
			[2],
			[3],
			[4]
		]);
	});
});

describe('sessions', () => {
	const list = [
		prompt({
			id: 12,
			received_at: '2026-11-05T10:05:00Z',
			fields: { session_id: 's1', session_title: 'Fix login' },
			context: { git_branch: 'fix/login' },
			summary: { response_s: 60, cost_usd: 0.25, ended_by: 'StopFailure', files_written: ['/a'] }
		}),
		prompt({
			id: 11,
			received_at: '2026-11-05T10:00:00Z',
			fields: { session_id: 's1', permission_mode: 'auto' },
			summary: {
				cost_usd: 0.75,
				files_written: ['/a', '/b'],
				tool_calls: { Bash: 2 },
				models: { 'claude-opus-5-5': 1 }
			}
		}),
		prompt({ id: 20, received_at: '2026-11-05T09:00:00Z', fields: { session_id: 's2' } })
	];
	const [first, second] = sessions(list);

	it('orders sessions by when they were last active', () =>
		expect([first.id, second.id]).toEqual(['s1', 's2']));

	it('tells the conversation oldest first', () =>
		expect(first.prompts.map((p) => p.id)).toEqual([11, 12]));

	it('spans from the first prompt to the end of the last turn', () => {
		expect(first.startedAt).toBe('2026-11-05T10:00:00Z');
		expect(first.endedAt).toBe('2026-11-05T10:06:00.000Z');
	});

	it('sums what the session cost and changed', () => {
		expect(first).toMatchObject({
			cost: 1,
			failed: 1,
			toolCalls: 2,
			filesWritten: 2,
			title: 'Fix login',
			branch: 'fix/login'
		});
		expect(first.models).toEqual(['claude-opus-5-5']);
		expect(first.permissionModes).toEqual(['auto']);
	});

	it('keeps a prompt without a session on its own', () => {
		const [solo] = sessions([prompt({ id: 7, fields: { session_id: undefined } })]);
		expect(solo.id).toBe('prompt-7');
	});
});

describe('projects', () => {
	it('groups by repository root and keeps the most informative name', () => {
		const days = byDay([], new Date(2026, 10, 4), new Date(2026, 10, 5));
		const list = projects(
			[
				prompt({
					received_at: '2026-11-05T10:00:00Z',
					context: { git_root: '/work/app', git_repo: 'me/app', git_branch: 'main' }
				}),
				prompt({
					received_at: '2026-11-05T11:00:00Z',
					context: { git_root: '/work/app', git_branch: 'feat' },
					summary: { cost_usd: 2 }
				}),
				prompt({
					received_at: '2026-11-04T11:00:00Z',
					fields: { cwd: '/tmp/x' },
					context: { git_root: undefined }
				})
			],
			days
		);
		expect(list.map((p) => [p.project.name, p.prompts])).toEqual([
			['me/app', 2],
			['x', 1]
		]);
		expect(list[0]).toMatchObject({
			cost: 2,
			branches: ['main', 'feat'],
			lastAt: '2026-11-05T11:00:00Z',
			daily: [0, 2]
		});
		expect(list[1].project.root).toBeUndefined();
	});
});

describe('projectOf', () => {
	it('falls back from repo to root folder to working folder', () => {
		expect(projectOf(prompt({ context: { git_root: '/w/app', git_repo: 'me/app' } })).name).toBe(
			'me/app'
		);
		expect(projectOf(prompt({ context: { git_root: '/w/app' } })).name).toBe('app');
		expect(
			projectOf(prompt({ context: { git_root: undefined }, fields: { cwd: '/w/notes' } }))
		).toMatchObject({ key: '/w/notes', name: 'notes' });
	});

	it('merges what two prompts know about one project', () => {
		const a = { key: '/w/app', name: 'app', root: '/w/app' };
		const b = { key: '/w/app', name: 'me/app', root: '/w/app', repo: 'me/app', host: 'github.com' };
		expect(mergeProject(a, b)).toEqual({ ...b });
	});
});

describe('lanes', () => {
	it('gives the busiest projects a lane each and folds the rest into "Other projects"', () => {
		const list = [
			...Array.from({ length: 3 }, (_, i) => prompt({ id: 100 + i, context: { git_root: '/a' } })),
			...Array.from({ length: 2 }, (_, i) => prompt({ id: 200 + i, context: { git_root: '/b' } })),
			prompt({ id: 300, context: { git_root: '/c' } }),
			prompt({ id: 400, context: { git_root: '/d' } })
		];
		const result = lanes(list, 3);
		expect(result.map((l) => [l.label, l.marks.length])).toEqual([
			['a', 3],
			['b', 2],
			['Other projects', 2]
		]);
		expect(result[0].root).toBe('/a');
	});

	it('keeps every project when they fit', () => {
		expect(lanes([prompt({ context: { git_root: '/a' } })], 6)).toHaveLength(1);
	});

	it('orders marks in time', () => {
		const [lane] = lanes([
			prompt({ id: 2, received_at: '2026-11-05T12:00:00Z' }),
			prompt({ id: 1, received_at: '2026-11-05T09:00:00Z' })
		]);
		expect(lane.marks.map((m) => m.id)).toEqual([1, 2]);
	});
});
