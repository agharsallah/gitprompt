import { readdirSync, readFileSync } from 'node:fs';
import { join, relative, sep } from 'node:path';
import { describe, expect, it } from 'vitest';

/**
 * Every URL the dashboard links to is built in lib/shared/links.ts, so deep links can be mapped
 * there later. This reads the source and names every other place that builds one: a route written
 * out in a string, an `#anchor` or `?query` glued to one, or a use of `resolve` from `$app/paths`.
 * Route ids kept as plain keys or types (`'/prompts'` alone) are not links and pass.
 */

const SRC = join(import.meta.dirname, '..', '..');
const ROUTES = 'sessions|projects|prompts|sources|search|settings|fields|design';

const FORBIDDEN: { why: string; pattern: RegExp }[] = [
	{ why: 'imports `$app/paths`', pattern: /from\s+['"]\$app\/paths['"]/ },
	{
		why: 'writes a route out in a string',
		pattern: new RegExp(`['"\`]/(${ROUTES})(?=[/?#$\\[])`)
	},
	{ why: 'builds an anchor to a prompt, session or tool', pattern: /#(prompt|session|tool)-/ },
	{ why: 'starts a query string', pattern: /['"`]\?[a-z]+=/ }
];

/** The source without its comments (documentation may name routes). */
function withoutComments(text: string): string {
	return text
		.replace(/\/\*[\s\S]*?\*\//g, '')
		.replace(/<!--[\s\S]*?-->/g, '')
		.replace(/(^|\s)\/\/.*$/gm, '$1');
}

function* sources(dir: string): Generator<string> {
	for (const entry of readdirSync(dir, { withFileTypes: true })) {
		const path = join(dir, entry.name);
		if (entry.isDirectory()) yield* sources(path);
		else if (/\.(ts|svelte)$/.test(entry.name)) yield path;
	}
}

const isChecked = (file: string) => {
	const name = relative(SRC, file).split(sep).join('/');
	return (
		name !== 'lib/shared/links.ts' &&
		name !== 'lib/api/generated.ts' &&
		!/\.test\.ts$/.test(name) &&
		!name.startsWith('lib/testing/')
	);
};

function offenders(): string[] {
	const found: string[] = [];
	for (const file of [...sources(SRC)].filter(isChecked)) {
		const lines = withoutComments(readFileSync(file, 'utf8')).split('\n');
		lines.forEach((line, i) => {
			for (const { why, pattern } of FORBIDDEN) {
				if (pattern.test(line)) {
					found.push(`${relative(SRC, file)}:${i + 1} ${why}: ${line.trim()}`);
				}
			}
		});
	}
	return found;
}

describe('route strings', () => {
	it('are built only in lib/shared/links.ts', () => {
		expect(offenders()).toEqual([]);
	});

	it('are found when a source writes one (the check can fail)', () => {
		const sample = "const href = `/prompts?id=${id}`;\nconst a = '/prompts';";
		const hits = sample
			.split('\n')
			.filter((line) => FORBIDDEN.some(({ pattern }) => pattern.test(line)));
		expect(hits).toEqual(['const href = `/prompts?id=${id}`;']);
	});
});
