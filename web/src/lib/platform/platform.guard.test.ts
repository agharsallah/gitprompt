import { readdirSync, readFileSync } from 'node:fs';
import { join, relative, sep } from 'node:path';
import { describe, expect, it } from 'vitest';

/**
 * The desktop shell's global object is read in lib/platform/ and nowhere else: every other file
 * asks `platform` (lib/platform/index.ts), which has an answer in a browser too. This reads the
 * source and names each other place that names the global, or the shell's packages, or one of the
 * shell's commands by its `plugin:` name.
 */

const SRC = join(import.meta.dirname, '..', '..');

const FORBIDDEN: { why: string; pattern: RegExp }[] = [
	{ why: "names the shell's global object", pattern: /__TAURI(?:_INTERNALS|_IPC)?__/ },
	{ why: "imports the shell's packages", pattern: /from\s+['"]@tauri-apps\// },
	{ why: 'calls a plugin command by name', pattern: /['"`]plugin:[a-z-]+\|/ }
];

/** The source without its comments (documentation may name the global). */
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
	return !name.startsWith('lib/platform/') && !/\.test\.ts$/.test(name);
};

function offenders(files: Iterable<string> = sources(SRC)): string[] {
	const found: string[] = [];
	for (const file of [...files].filter(isChecked)) {
		const lines = withoutComments(readFileSync(file, 'utf8')).split('\n');
		lines.forEach((line, i) => {
			for (const { why, pattern } of FORBIDDEN) {
				if (pattern.test(line))
					found.push(`${relative(SRC, file)}:${i + 1} ${why}: ${line.trim()}`);
			}
		});
	}
	return found;
}

describe('the desktop shell', () => {
	it('is reached only through lib/platform/', () => {
		expect(offenders()).toEqual([]);
	});

	it('is found when a source names it (the check can fail)', () => {
		const lines = [
			'const t = window.__TAURI__.core;',
			"import { invoke } from '@tauri-apps/api/core';",
			"invoke('plugin:gitprompt|status');",
			'const fine = platform.hideWindow();'
		];
		const hits = lines.filter((line) => FORBIDDEN.some(({ pattern }) => pattern.test(line)));
		expect(hits).toEqual(lines.slice(0, 3));
	});
});
