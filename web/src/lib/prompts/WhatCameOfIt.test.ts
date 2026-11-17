import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import type { CommitsAfter } from '#lib/api/types.ts';
import WhatCameOfIt from '#lib/prompts/WhatCameOfIt.svelte';
import { projectId } from '#lib/stories/story.ts';
import { prompt } from '#lib/testing/builders.ts';

const draw = (commits: CommitsAfter, cost: number | null, written = ['/work/app/src/a.ts']) =>
	render(WhatCameOfIt, {
		props: {
			id: 'came',
			prompt: {
				...prompt({ summary: { files_written: written }, facts: { cost_usd: cost } }),
				commits
			}
		}
	}).body;

const committed: CommitsAfter = {
	commits: [{ hash: 'a1b2c3d', subject: 'Fix the test', author: 'Ada', time: '' }],
	diffstat: ' 1 file changed, 4 insertions(+), 2 deletions(-)',
	until: 'a1b2c3d'
};

describe('what came of a turn', () => {
	it('links each commit to its row on the project Commits tab', () => {
		const html = draw(committed, 1.84);
		expect(html).toContain('Wrote 1 file (+4 −2), 1 commit, $1.84');
		expect(html).toContain(`href="/projects/${projectId('/work/app')}?tab=commits#commit-a1b2c3d"`);
		expect(html).toContain('Fix the test');
		expect(html).not.toContain('No commit followed');
	});

	it('says no commit followed when none did', () => {
		const html = draw({ until: 'abc' }, 0.12);
		expect(html).toContain('Wrote 1 file, no commit, $0.12');
		expect(html).toContain('No commit followed');
		expect(html).toContain('src/a.ts');
	});

	it('leaves the cost out for a tool that reports none', () => {
		const html = draw(committed, null);
		expect(html).toContain('Wrote 1 file (+4 −2), 1 commit<');
		expect(html).not.toContain('$');
	});

	it('says so, and nothing more, when the turn produced nothing', () => {
		const html = draw({ until: 'abc' }, null, []);
		expect(html).toContain('Nothing was written or committed');
		expect(html).not.toContain('No commit followed');
	});
});
