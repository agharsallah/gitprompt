import { describe, expect, it } from 'vitest';
import { diffCounts } from '#lib/sources/diff.ts';

describe('diffCounts', () => {
	it('counts added and removed lines, not the file headers', () => {
		const diff = [
			'--- ~/.claude/settings.json',
			'+++ ~/.claude/settings.json',
			'@@ -1,3 +1,4 @@',
			' {',
			'-  "theme": "dark"',
			'+  "theme": "dark",',
			'+  "hooks": {}',
			' }'
		].join('\n');
		expect(diffCounts(diff)).toEqual({ added: 2, removed: 1 });
	});

	it('is zero for no change', () => {
		expect(diffCounts('')).toEqual({ added: 0, removed: 0 });
	});
});
