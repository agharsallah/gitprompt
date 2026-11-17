import { describe, expect, it } from 'vitest';
import type { CommitsAfter } from '#lib/api/types.ts';
import { cameOfIt, commitRowHref, NOTHING, type TurnResult } from '#lib/prompts/cameOfIt.ts';
import { projectId } from '#lib/stories/story.ts';
import { prompt } from '#lib/testing/builders.ts';

const files = (n: number) => Array.from({ length: n }, (_, i) => `/work/app/src/f${i + 1}.ts`);
const commit = (hash: string) => ({ hash, subject: `Commit ${hash}`, author: 'Ada', time: '' });

function turn({
	written = [] as string[],
	commits = {} as CommitsAfter,
	cost = null as number | null
} = {}): TurnResult {
	return { ...prompt({ summary: { files_written: written }, facts: { cost_usd: cost } }), commits };
}

describe('the sentence', () => {
	it('says files, lines, commits and cost', () => {
		const view = cameOfIt(
			turn({
				written: files(6),
				commits: {
					commits: [commit('a1'), commit('b2')],
					diffstat: ' 6 files changed, 212 insertions(+), 40 deletions(-)',
					until: 'b2'
				},
				cost: 1.84
			})
		);
		expect(view.sentence).toBe('Wrote 6 files (+212 −40), 2 commits, $1.84');
	});

	it('says "no commit" and no lines when nothing was committed', () => {
		expect(cameOfIt(turn({ written: files(1), cost: 0.12 })).sentence).toBe(
			'Wrote 1 file, no commit, $0.12'
		);
	});

	it('leaves the cost out when the tool reports none', () => {
		expect(cameOfIt(turn({ written: files(2) })).sentence).toBe('Wrote 2 files, no commit');
	});

	it('says what the commits changed when the turn wrote nothing itself', () => {
		const view = cameOfIt(
			turn({
				commits: {
					commits: [commit('a1')],
					diffstat: ' 1 file changed, 3 insertions(+)',
					until: 'a1'
				}
			})
		);
		expect(view.sentence).toBe('Changed 1 file (+3 −0), 1 commit');
	});

	it('says so when nothing was written or committed', () => {
		const view = cameOfIt(turn({ cost: 0.5 }));
		expect(view.sentence).toBe(NOTHING);
		expect(view.empty).toBe(true);
	});
});

describe('the files', () => {
	it('lists the first five, relative to the repository, and counts the rest', () => {
		const view = cameOfIt(turn({ written: files(12) }));
		expect(view.files).toEqual(['src/f1.ts', 'src/f2.ts', 'src/f3.ts', 'src/f4.ts', 'src/f5.ts']);
		expect(view.more).toBe(7);
	});

	it('has nothing more when five or fewer were written', () => {
		expect(cameOfIt(turn({ written: files(5) })).more).toBe(0);
	});
});

describe('the commits', () => {
	it('link to their row on the project Commits tab', () => {
		const view = cameOfIt(turn({ commits: { commits: [commit('a1b2c3d')], until: 'a1b2c3d' } }));
		expect(view.commits[0].href).toBe(
			`/projects/${projectId('/work/app')}?tab=commits#commit-a1b2c3d`
		);
	});

	it('link nowhere outside a repository', () => {
		expect(commitRowHref(undefined, 'a1')).toBeUndefined();
	});
});
