// Finds the time dependencies of rules.mjs in the backend and the dashboard: every source file git
// knows (tracked, or new and not ignored), line by line, comments left out.
import { execFileSync } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';

import { policyOf, rulesFor } from './rules.mjs';

/** The folders scanned: the Rust workspace's crates and apps, the dashboard's source. */
export const ROOTS = ['backend/crates', 'backend/apps', 'web/src'];
const LANGUAGES = [
	{ language: 'rust', file: /\.rs$/ },
	{ language: 'web', file: /\.(?:ts|js|svelte)$/ }
];

/** `rust`, `web`, or null for a file no rule reads. */
export function languageOf(file) {
	return LANGUAGES.find((l) => l.file.test(file))?.language ?? null;
}

/** A test file or test helper as a whole: under a `test`/`tests`/`benches` folder, or `*.test.ts`. */
export function isTestFile(file) {
	return /(?:^|\/)(?:tests?|benches)\//.test(file) || /\.(?:test|spec)\.[jt]s$/.test(file);
}

/** A line's code: '' for a comment line, else the line without a trailing `//` comment. */
export function codeOf(line) {
	const trimmed = line.trim();
	if (/^(?:\/\/|\/\*|\*|<!--)/.test(trimmed)) return '';
	// `//` after a `:` is a URL in a string (`http://`), not a comment.
	return line.replace(/(^|[^:])\/\/.*$/, '$1');
}

/**
 * The time dependencies in one file's text: [{ rule, file, line (1-based), scope, policy (`block`
 * or `report`, rules.mjs), text }], one per rule and line. In Rust, every line from the first
 * `#[cfg(test)]` on is test code (test modules close a file here).
 */
export function scanText(file, text) {
	const language = languageOf(file);
	if (!language) return [];
	let scope = isTestFile(file) ? 'test' : 'code';
	const findings = [];
	text.split('\n').forEach((line, index) => {
		if (language === 'rust' && /^\s*#\[cfg\(test\)\]/.test(line)) scope = 'test';
		const code = codeOf(line);
		if (!code.trim()) return;
		for (const rule of rulesFor(language, scope)) {
			const except = rule[`${language}Except`];
			if (rule[language].test(except ? code.replace(except, '') : code)) {
				findings.push({ rule: rule.id, file, line: index + 1, scope, policy: policyOf(rule, file, scope), text: line.trim() });
			}
		}
	});
	return findings;
}

/** Source files under `roots`, sorted: tracked or new, never ignored (target/, node_modules/). */
export function sourceFiles(repo, roots = ROOTS) {
	const listed = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard', '-z', '--', ...roots], { cwd: repo, encoding: 'utf8' });
	return [...new Set(listed.split('\0'))].filter((file) => file && languageOf(file)).sort();
}

/** Every time dependency under `roots` of the repository at `repo`. */
export async function scan(repo, roots = ROOTS) {
	const findings = await Promise.all(
		sourceFiles(repo, roots).map(async (file) => {
			// A tracked file deleted in the working tree is still listed: it has nothing to scan.
			const text = await readFile(join(repo, file), 'utf8').catch((e) => (e.code === 'ENOENT' ? '' : Promise.reject(e)));
			return scanText(file, text);
		})
	);
	return findings.flat();
}
