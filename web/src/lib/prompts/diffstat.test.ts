import { describe, expect, it } from 'vitest';
import { parseDiffstat } from '#lib/prompts/diffstat.ts';

describe('parseDiffstat', () => {
	it('reads files, insertions and deletions', () => {
		expect(parseDiffstat(' 3 files changed, 120 insertions(+), 4 deletions(-)')).toEqual({
			files: 3,
			insertions: 120,
			deletions: 4
		});
	});

	it('counts a part git leaves out as zero, singular or plural', () => {
		expect(parseDiffstat(' 1 file changed, 1 insertion(+)')).toEqual({
			files: 1,
			insertions: 1,
			deletions: 0
		});
	});

	it('is null without a line', () => {
		expect(parseDiffstat(undefined)).toBeNull();
		expect(parseDiffstat('')).toBeNull();
	});
});
