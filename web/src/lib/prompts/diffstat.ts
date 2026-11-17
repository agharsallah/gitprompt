// A range's `git diff --shortstat` line (the daemon's `commits.diffstat`) as numbers. Pure, tested
// in diffstat.test.ts.

export type Diffstat = { files: number; insertions: number; deletions: number };

/** "3 files changed, 120 insertions(+), 4 deletions(-)" -> numbers; null without a line. */
export function parseDiffstat(stat: string | undefined): Diffstat | null {
	if (!stat) return null;
	const num = (re: RegExp) => Number(stat.match(re)?.[1] ?? 0);
	return {
		files: num(/(\d+) files? changed/),
		insertions: num(/(\d+) insertions?\(\+\)/),
		deletions: num(/(\d+) deletions?\(-\)/)
	};
}
