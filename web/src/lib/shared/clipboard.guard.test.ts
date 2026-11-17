import { readdirSync, readFileSync } from 'node:fs';
import { join, relative, sep } from 'node:path';
import { describe, expect, it } from 'vitest';

/**
 * Every copy goes through lib/shared/clipboard.ts (CopyButton, the palette), so every copy is
 * counted and every one starts the way Safari and the desktop web view accept. This reads the
 * source and names every other file that touches the browser's clipboard directly.
 */

const SRC = join(import.meta.dirname, '..', '..');
const PATTERN = /navigator\.clipboard|\bClipboardItem\b|execCommand\s*\(\s*['"](copy|cut)['"]/;

/** The source without its comments (documentation may name the clipboard). */
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
	return name !== 'lib/shared/clipboard.ts' && !/\.test\.ts$/.test(name);
};

function offenders(): string[] {
	const found: string[] = [];
	for (const file of [...sources(SRC)].filter(isChecked)) {
		withoutComments(readFileSync(file, 'utf8'))
			.split('\n')
			.forEach((line, i) => {
				if (PATTERN.test(line)) found.push(`${relative(SRC, file)}:${i + 1} ${line.trim()}`);
			});
	}
	return found;
}

describe('the clipboard', () => {
	it('is touched only in lib/shared/clipboard.ts', () => {
		expect(offenders()).toEqual([]);
	});

	it('is found when a source writes to it (the check can fail)', () => {
		const sample = [
			'await navigator.clipboard.writeText(text);',
			'new ClipboardItem({})',
			"document.execCommand('copy');",
			'const copied = true;'
		];
		expect(sample.filter((line) => PATTERN.test(line))).toHaveLength(3);
	});
});
