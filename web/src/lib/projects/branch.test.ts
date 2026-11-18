import { describe, expect, it } from 'vitest';
import { branchUrl, copyBranch, currentBranch, type CopyDeps } from '#lib/projects/branch.ts';
import type { Pending } from '#lib/shared/clipboard.ts';
import type { UsageAction } from '#lib/shared/usage-actions.ts';

const BLOCK = '### Prompts behind feat/x\n\n1 prompt · 1 commit\n';

/** Deps that answer with `status`, copy unless `refuse`, and record what was copied and said. */
function page(status = 200, refuse = false) {
	const asked: string[] = [];
	const copied: { text: string; action: UsageAction }[] = [];
	const said: [string, string][] = [];
	const deps: CopyDeps = {
		fetcher: (input) => {
			if (typeof input !== 'string') throw new Error('the block is asked for by its URL');
			asked.push(input);
			const body = status === 200 ? BLOCK : 'project not found';
			return Promise.resolve(new Response(body, { status }));
		},
		copy: async (text: Pending, action: UsageAction) => {
			try {
				copied.push({ text: await text, action });
			} catch {
				return false;
			}
			return !refuse;
		},
		say: (message, tone) => said.push([message, tone])
	};
	return { deps, asked, copied, said };
}

describe('the prompts behind a branch', () => {
	it('are asked for by project and branch, both encoded', () => {
		expect(branchUrl('L3dvcmsvYXBw', 'feat/x y')).toBe(
			'/api/export/branch?project=L3dvcmsvYXBw&branch=feat%2Fx+y'
		);
	});

	it('copy the block the daemon renders, counted as copy.branch, and say so', async () => {
		const { deps, asked, copied, said } = page();
		expect(await copyBranch('p1', 'feat/x', deps)).toBe(true);
		expect(asked).toEqual(['/api/export/branch?project=p1&branch=feat%2Fx']);
		expect(copied).toEqual([{ text: BLOCK, action: 'copy.branch' }]);
		expect(said).toEqual([['Copied the prompts behind feat/x', 'info']]);
	});

	it('say the copy failed when the daemon refuses, and copy nothing', async () => {
		const { deps, copied, said } = page(404);
		expect(await copyBranch('p1', 'feat/x', deps)).toBe(false);
		expect(copied).toEqual([]);
		expect(said).toEqual([['Could not copy the prompts behind the branch', 'error']]);
	});

	it('say the copy failed when the clipboard refuses', async () => {
		const { deps, said } = page(200, true);
		expect(await copyBranch('p1', 'feat/x', deps)).toBe(false);
		expect(said).toEqual([['Could not copy the prompts behind the branch', 'error']]);
	});
});

describe('the current branch of a project', () => {
	const phase = (...branches: (string | null)[]) => ({
		items: branches.map((branch) => ({ branch }))
	});

	it('is the branch of the latest prompt that names one', () => {
		expect(currentBranch({ phases: [phase('main', 'feat'), phase('fix', null)] })).toBe('fix');
	});

	it('is none in a folder that is not a repository', () => {
		expect(currentBranch({ phases: [phase(null)] })).toBeUndefined();
		expect(currentBranch({ phases: [] })).toBeUndefined();
	});
});
