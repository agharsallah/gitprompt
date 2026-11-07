// Facts about a unified diff, for the consent review. Pure, tested in diff.test.ts.

/** Lines added and removed, not counting the `+++`/`---` file headers. */
export function diffCounts(diff: string): { added: number; removed: number } {
	let added = 0;
	let removed = 0;
	for (const line of diff.split('\n')) {
		if (line.startsWith('+++') || line.startsWith('---')) continue;
		if (line.startsWith('+')) added++;
		else if (line.startsWith('-')) removed++;
	}
	return { added, removed };
}
