// Every function's cognitive complexity (sonarjs) and length, and every file's size, measured by the
// ESLint rules the gate runs with their limits lowered to 0 so each reports its value. The limits
// themselves are read from the resolved config (web/eslint.config.js).
import { join, relative } from 'node:path';

import { complexityPage } from '../../lib/complexity.mjs';
import { webSync } from '../../lib/shared.mjs';

/** Rules measured (value read from the message) rather than only counted. */
const MEASURED = ['sonarjs/cognitive-complexity', 'max-lines-per-function', 'max-lines'];
/** Other complexity limits: their findings are what the gate fails on. */
const LIMITED = ['complexity', 'max-depth', 'max-params', 'max-nested-callbacks'];
const TEST = /\.test\.ts$/;

export default {
	title: 'Complexity',
	description: 'Function cognitive complexity and length, file sizes, and the other complexity limits ESLint enforces.',
	needs: [{ command: 'pnpm', install: 'https://pnpm.io/installation' }],
	async run(ctx) {
		await webSync(ctx);
		const limits = await configuredLimits(ctx);
		const out = join(ctx.artifacts, 'eslint-complexity.json');
		const measureAll = [
			['sonarjs/cognitive-complexity', ['warn', 0]],
			['max-lines-per-function', ['warn', { max: 0, skipBlankLines: true, skipComments: true }]],
			['max-lines', ['warn', { max: 0, skipBlankLines: true, skipComments: true }]]
		].flatMap(([rule, setting]) => ['--rule', JSON.stringify({ [rule]: setting })]);
		await ctx.exec('pnpm', ['--dir', 'web', 'exec', 'eslint', '.', ...measureAll, '--format', 'json', '--output-file', out], {
			label: 'eslint (every function measured)',
			allowFailure: true
		});
		if (!(await ctx.exists(out))) throw new Error('eslint wrote no results; see the log');
		const messages = (await ctx.readJson(out)).flatMap((file) => file.messages.map((m) => ({ ...m, file: relative(ctx.web, file.filePath) })));
		return complexityPage({
			functions: functions(messages),
			files: messages.filter((m) => m.ruleId === 'max-lines').map((m) => ({ file: m.file, unit: unitOf(m.file), lines: valueIn(m.message), limit: limits.file })),
			limits: { complexity: limits.complexity, lines: limits.lines },
			findings: messages.filter((m) => LIMITED.includes(m.ruleId)).map((m) => ({ place: `${m.file}:${m.line}`, rule: m.ruleId, message: m.message })),
			measure:
				"Complexity is Sonar's cognitive complexity: each break in the flow counts, and counts more the deeper it is nested. Lines skip blanks and comments. Test files count for file size only (a test file is one long `describe`). ESLint allows no exemption in place: a limit changes in eslint.config.js, with the user."
		});
	}
};

/** The limits as configured, from ESLint's resolved config for a source file. */
async function configuredLimits(ctx) {
	const printed = await ctx.exec('pnpm', ['--dir', 'web', 'exec', 'eslint', '--print-config', 'src/lib/shared/format.ts'], { label: 'eslint --print-config', dataOnStdout: true });
	const rules = JSON.parse(printed.stdout).rules;
	const option = (rule) => rules[rule]?.[1];
	return {
		complexity: option('sonarjs/cognitive-complexity'),
		lines: option('max-lines-per-function')?.max,
		file: option('max-lines')?.max
	};
}

/** The first number in a message: "…too many lines (422)…", "…Complexity from 21 to…". */
const valueIn = (message) => Number(message.match(/(\d+)/)?.[1] ?? 0);

/** `src/lib/prompts/x.ts` → `lib/prompts`; `src/routes/fields/+page.svelte` → `routes/fields`. */
function unitOf(file) {
	return file.split('/').slice(1, 3).join('/');
}

/**
 * One entry per function of shipped code: max-lines-per-function names every function (its length);
 * sonarjs reports the complexity of those above 0 at the same line.
 */
function functions(messages) {
	const shipped = messages.filter((m) => !TEST.test(m.file));
	const complexity = new Map(shipped.filter((m) => m.ruleId === 'sonarjs/cognitive-complexity').map((m) => [`${m.file}:${m.line}`, valueIn(m.message)]));
	return shipped
		.filter((m) => m.ruleId === 'max-lines-per-function')
		.map((m) => {
			const place = `${m.file}:${m.line}`;
			return {
				name: m.message.match(/'([^']+)'/)?.[1] ?? `(${m.message.split(' has ')[0].toLowerCase()})`,
				place,
				unit: unitOf(m.file),
				complexity: complexity.get(place) ?? 0,
				lines: Number(m.message.match(/\((\d+)\)/)?.[1] ?? 0)
			};
		});
}
