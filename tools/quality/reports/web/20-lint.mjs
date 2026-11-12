// What `pnpm lint` and `pnpm check` fail on: ESLint findings (by rule and by place), svelte-check
// type errors and warnings, and files Prettier would reformat.
import { join, relative } from 'node:path';

import { groupBy, kpis, note, plural, table, toneZero } from '../../lib/spec.mjs';

/** Findings listed one by one (the rest are counted). */
const LISTED = 300;

export default {
	title: 'Lint & types',
	description: 'ESLint findings, svelte-check type errors and Prettier formatting: what `just lint` and CI fail on.',
	needs: [{ command: 'pnpm', install: 'https://pnpm.io/installation' }],
	async run(ctx) {
		await ctx.exec('pnpm', ['--dir', 'web', 'exec', 'svelte-kit', 'sync'], { label: 'svelte-kit sync' });
		const eslintFile = join(ctx.artifacts, 'eslint.json');
		await ctx.exec('pnpm', ['--dir', 'web', 'exec', 'eslint', '.', '--format', 'json', '--output-file', eslintFile], { label: 'eslint .', allowFailure: true });
		if (!(await ctx.exists(eslintFile))) throw new Error('eslint wrote no results; see the log');
		const eslint = (await ctx.readJson(eslintFile)).flatMap((file) => file.messages.map((m) => ({ ...m, file: relative(ctx.web, file.filePath) })));
		const files = (await ctx.readJson(eslintFile)).length;
		const check = await ctx.exec('pnpm', ['--dir', 'web', 'exec', 'svelte-check', '--tsconfig', './tsconfig.json', '--output', 'machine'], { label: 'svelte-check (types)', allowFailure: true });
		const types = parseSvelteCheck(check.stdout);
		const prettier = await ctx.exec('pnpm', ['--dir', 'web', 'exec', 'prettier', '--list-different', '.'], { label: 'prettier --list-different', allowFailure: true });
		const unformatted = prettier.stdout.split('\n').map((line) => line.trim()).filter(Boolean);
		return page(eslint, files, types, unformatted);
	}
};

/** `<time> ERROR "src/x.svelte" 3:5 "message"` lines. */
export function parseSvelteCheck(stdout) {
	return [...stdout.matchAll(/^\d+ (ERROR|WARNING) "(.+?)" (\d+):(\d+) "(.*)"$/gm)].map((m) => ({ level: m[1].toLowerCase(), file: m[2], line: Number(m[3]), message: m[5].replaceAll('\\n', ' ') }));
}

function page(eslint, files, types, unformatted) {
	const errors = eslint.filter((m) => m.severity === 2).length;
	const warnings = eslint.length - errors;
	const typeErrors = types.filter((t) => t.level === 'error').length;
	const typeWarnings = types.length - typeErrors;
	const failing = errors + warnings + typeErrors + unformatted.length;
	const sections = [
		kpis([
			{ label: 'ESLint errors', value: errors, tone: toneZero(errors) },
			{ label: 'ESLint warnings', value: warnings, tone: warnings ? 'warn' : 'good' },
			{ label: 'Type errors', value: typeErrors, tone: toneZero(typeErrors) },
			{ label: 'Type warnings', value: typeWarnings, tone: typeWarnings ? 'warn' : 'good' },
			{ label: 'Files to format', value: unformatted.length, tone: toneZero(unformatted.length) },
			{ label: 'Files linted', value: files }
		])
	];
	sections.push(
		failing + typeWarnings === 0
			? note('Clean: ESLint, svelte-check and Prettier have nothing to report. Silencing comments are rejected by the config (no-lint-silencing).', 'good')
			: note('Fix the cause, never silence it: eslint-disable, @ts-ignore and `any` are rejected (web/eslint.config.js).', failing ? 'bad' : 'warn')
	);
	if (eslint.length) {
		sections.push(
			table(
				'ESLint by rule',
				[
					{ key: 'rule', label: 'Rule', mono: true },
					{ key: 'count', label: 'Findings', align: 'right' },
					{ key: 'example', label: 'Example' }
				],
				groupBy(eslint, (m) => m.ruleId || 'parse error').map(([rule, list]) => ({ rule, count: list.length, example: list[0].message }))
			),
			table(
				'ESLint findings',
				[
					{ key: 'place', label: 'Where', mono: true },
					{ key: 'rule', label: 'Rule', mono: true },
					{ key: 'message', label: 'Message' }
				],
				eslint.slice(0, LISTED).map((m) => ({ place: `${m.file}:${m.line}:${m.column}`, rule: { text: m.ruleId || 'parse error', tone: m.severity === 2 ? 'bad' : 'warn' }, message: m.message })),
				eslint.length > LISTED ? `First ${LISTED} of ${eslint.length}.` : undefined
			)
		);
	}
	if (types.length) {
		sections.push(
			table(
				'Type diagnostics (svelte-check)',
				[
					{ key: 'place', label: 'Where', mono: true },
					{ key: 'level', label: 'Level' },
					{ key: 'message', label: 'Message' }
				],
				types.map((t) => ({ place: `${t.file}:${t.line}`, level: { text: t.level, tone: t.level === 'error' ? 'bad' : 'warn' }, message: t.message }))
			)
		);
	}
	if (unformatted.length) sections.push(table('Files Prettier would change (run `just fmt`)', [{ key: 'file', label: 'File', mono: true }], unformatted.map((file) => ({ file }))));
	return {
		headline: { value: failing, label: failing === 1 ? 'finding' : 'findings', tone: failing ? 'bad' : typeWarnings ? 'warn' : 'good' },
		summary: `${plural(errors + warnings, 'ESLint finding')}, ${plural(typeErrors, 'type error')}, ${plural(unformatted.length, 'file')} to format · ${files} files linted`,
		sections
	};
}
