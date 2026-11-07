import { describe, expect, it } from 'vitest';
import { projectSummaries } from '#lib/projects/summary.ts';
import { bucket } from '#lib/testing/builders.ts';

const slot = (key: string) => ({ key, date: new Date(), bucket: null });

describe('projectSummaries', () => {
	it('joins the three splits of each project', () => {
		const byDay = [
			bucket({
				key: '/r/app',
				detail: 'me/app',
				prompts: 3,
				failed: 1,
				last_at: 'T',
				parts: [
					bucket({ key: '2026-11-02', prompts: 2 }),
					bucket({ key: '2026-11-04', prompts: 1 })
				]
			}),
			bucket({ key: '/tmp/notes', prompts: 1, parts: [] })
		];
		const byKind = [
			bucket({
				key: '/r/app',
				parts: [bucket({ key: 'shell', tool_calls: 4 }), bucket({ key: 'read', tool_calls: 2 })]
			})
		];
		const byBranch = [
			bucket({ key: '/r/app', parts: [bucket({ key: 'main' }), bucket({ key: null })] })
		];
		const [app, notes] = projectSummaries(
			byDay,
			byKind,
			byBranch,
			['2026-11-02', '2026-11-03', '2026-11-04'].map(slot)
		);
		expect(app).toMatchObject({
			key: '/r/app',
			name: 'me/app',
			git: true,
			failed: 1,
			branches: ['main'],
			daily: [2, 0, 1]
		});
		expect(app.kinds).toEqual([
			{ kind: 'read', count: 2 },
			{ kind: 'shell', count: 4 }
		]);
		expect(notes).toMatchObject({ name: 'notes', git: false, kinds: [], daily: [0, 0, 0] });
	});
});
