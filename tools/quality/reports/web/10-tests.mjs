// The dashboard's unit tests (vitest) with V8 coverage of src/lib: results per test file, failures,
// coverage per folder and file, and vitest's line-by-line report.
import { join, relative } from 'node:path';

import { bars, checks, coverageCell, embed, groupBy, kpis, ms, note, pct, plural, table, tone } from '../../lib/spec.mjs';

export default {
	title: 'Tests & coverage',
	description: 'Vitest unit tests with V8 coverage of src/lib (the thresholds in vite.config.ts apply).',
	needs: [{ command: 'pnpm', install: 'https://pnpm.io/installation' }],
	async run(ctx) {
		const coverageDir = join(ctx.artifacts, 'coverage');
		const results = join(ctx.artifacts, 'vitest.json');
		const run = await ctx.exec(
			'pnpm',
			['--dir', 'web', 'exec', 'vitest', 'run', '--coverage', '--coverage.reporter=json-summary', '--coverage.reporter=html', `--coverage.reportsDirectory=${coverageDir}`, '--reporter=json', `--outputFile=${results}`],
			{ label: 'vitest run --coverage', allowFailure: true }
		);
		if (!(await ctx.exists(results))) throw new Error(`vitest wrote no results (exit ${run.code}); see the log`);
		const tests = await ctx.readJson(results);
		const summaryPath = join(coverageDir, 'coverage-summary.json');
		const coverage = (await ctx.exists(summaryPath)) ? await ctx.readJson(summaryPath) : null;
		const thresholds = [...`${run.stdout}\n${run.stderr}`.matchAll(/^ERROR: (Coverage for .+)$/gm)].map((m) => m[1]);
		return page(ctx, tests, coverage, { exitCode: run.code, thresholds }, join(coverageDir, 'index.html'));
	}
};

function page(ctx, tests, coverage, { exitCode, thresholds }, html) {
	const rel = (path) => relative(ctx.web, path);
	const failed = tests.numFailedTests;
	const total = coverage?.total;
	const share = (metric) => (total ? total[metric].pct / 100 : null);
	const files = coverage ? Object.entries(coverage).filter(([key]) => key !== 'total').map(([path, data]) => ({ path: rel(path), lines: data.lines })) : [];
	const thresholdMissed = exitCode !== 0 && failed === 0;

	const sections = [
		kpis([
			{ label: 'Tests passed', value: tests.numPassedTests, tone: failed ? 'bad' : 'good' },
			{ label: 'Failed', value: failed, tone: failed ? 'bad' : 'good' },
			{ label: 'Skipped', value: tests.numPendingTests + tests.numTodoTests, tone: tests.numPendingTests ? 'warn' : 'neutral' },
			{ label: 'Test files', value: tests.numTotalTestSuites ? tests.testResults.length : 0 },
			{ label: 'Lines', value: pct(share('lines')), tone: tone(share('lines'), 0.8, 0.6) },
			{ label: 'Branches', value: pct(share('branches')), tone: tone(share('branches'), 0.8, 0.6) },
			{ label: 'Functions', value: pct(share('functions')), tone: tone(share('functions'), 0.8, 0.6) }
		])
	];
	if (failed) sections.push(note(`${plural(failed, 'test')} failed: \`just test\` and CI fail too.`, 'bad'));
	if (thresholdMissed) {
		sections.push(note(`Every test passed, but ${thresholds.length ? 'coverage misses a threshold in vite.config.ts, so `pnpm test:coverage` fails' : `vitest exited ${exitCode} (see the log)`}.`, 'bad'));
		if (thresholds.length) sections.push(checks('Coverage thresholds (vite.config.ts)', thresholds.map((text) => ({ label: text, ok: false }))));
	}
	const failures = tests.testResults.flatMap((file) => file.assertionResults.filter((a) => a.status === 'failed').map((a) => ({ file: rel(file.name), test: a.fullName, message: (a.failureMessages[0] || '').split('\n')[0] })));
	if (failures.length) sections.push(table('Failing tests', [{ key: 'file', label: 'File', mono: true }, { key: 'test', label: 'Test' }, { key: 'message', label: 'Error' }], failures));
	if (files.length) {
		const folders = groupBy(files, (file) => file.path.split('/').slice(0, 3).join('/'));
		sections.push(
			bars(
				'Line coverage by folder',
				folders
					.map(([folder, list]) => {
						const covered = list.reduce((t, f) => t + f.lines.covered, 0);
						const count = list.reduce((t, f) => t + f.lines.total, 0);
						return { label: folder, value: count ? covered / count : 0, text: `${pct(count ? covered / count : 0)} · ${covered}/${count}`, tone: tone(count ? covered / count : 0, 0.8, 0.6) };
					})
					.sort((a, b) => a.value - b.value),
				'Measured: src/lib/**/*.ts (vite.config.ts). Svelte components are not unit-tested here.'
			)
		);
	}
	sections.push(
		table(
			'Test files',
			[
				{ key: 'file', label: 'File', mono: true },
				{ key: 'tests', label: 'Tests', align: 'right' },
				{ key: 'failed', label: 'Failed', align: 'right' },
				{ key: 'time', label: 'Time', align: 'right' }
			],
			tests.testResults.map((file) => {
				const failedHere = file.assertionResults.filter((a) => a.status === 'failed').length;
				const time = file.endTime - file.startTime;
				return { file: rel(file.name), tests: file.assertionResults.length, failed: { text: failedHere, tone: failedHere ? 'bad' : undefined, sort: failedHere }, time: { text: ms(time), sort: time } };
			})
		)
	);
	if (files.length) {
		sections.push(
			table(
				'Files by coverage, least covered first',
				[
					{ key: 'path', label: 'File', mono: true },
					{ key: 'lines', label: 'Lines', align: 'right' },
					{ key: 'coverage', label: 'Coverage', align: 'right' }
				],
				files.sort((a, b) => a.lines.pct - b.lines.pct).map((file) => ({ path: file.path, lines: file.lines.total, coverage: coverageCell(file.lines.covered, file.lines.total) }))
			),
			embed('Line by line (vitest, V8)', ctx.fromHub(html))
		);
	}
	const bad = failed > 0 || thresholdMissed;
	return {
		headline: failed ? { value: failed, label: failed === 1 ? 'test failing' : 'tests failing', tone: 'bad' } : { value: pct(share('lines')), label: 'lines covered', tone: bad ? 'bad' : tone(share('lines'), 0.8, 0.6) },
		summary: `${plural(tests.numPassedTests, 'test')} passed${failed ? `, ${failed} failed` : ''} in ${plural(tests.testResults.length, 'file')}${total ? ` · ${total.lines.covered} of ${total.lines.total} lines covered` : ''}${thresholdMissed ? ' · coverage threshold missed' : ''}`,
		sections
	};
}
