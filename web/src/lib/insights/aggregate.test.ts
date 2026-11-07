import { describe, expect, it } from 'vitest';
import { cacheShare, sessions, tokensOf } from '#lib/insights/aggregate.ts';
import { mergeProject, projectOf } from '#lib/insights/project.ts';
import { prompt } from '#lib/testing/builders.ts';

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
