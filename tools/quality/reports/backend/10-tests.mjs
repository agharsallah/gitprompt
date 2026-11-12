// Every Rust test, run once under cargo llvm-cov (`just cov`): which passed, which failed, and the
// lines they reach, per crate and per file, with llvm-cov's line-by-line report.
import { join } from 'node:path';

import { bars, coverageCell, embed, kpis, note, pct, plural, table, tone } from '../../lib/spec.mjs';

export default {
	title: 'Tests & coverage',
	description: 'Every Rust test (unit, integration, end-to-end) under cargo llvm-cov: results per crate and the lines they reach.',
	needs: [{ command: 'cargo-llvm-cov', install: 'cargo install cargo-llvm-cov' }],
	async run(ctx) {
		const result = await ctx.exec('just', ['cov'], { label: 'running every test with coverage (just cov)', allowFailure: true });
		// cargo prints "Running <binary>" on stderr and its results on stdout: parse them in order.
		const runs = parseRuns(result.output, await packagesByTarget(ctx));
		if (runs.length === 0) throw new Error(`no test ran: just cov exited ${result.code}; see the log`);
		const coverage = result.code === 0 ? await ctx.readJson(join(ctx.backend, 'target/llvm-cov/summary.json')) : null;
		return page(ctx, runs, coverage);
	}
};

/** Target name (as cargo prints it, `_` for `-`) → its package: the CLI's binary `gitprompt` is gitprompt-cli. */
async function packagesByTarget(ctx) {
	const { stdout } = await ctx.exec('cargo', ['metadata', '--format-version', '1', '--no-deps'], { cwd: ctx.backend, label: 'reading the workspace (cargo metadata)' });
	const map = new Map();
	for (const pkg of JSON.parse(stdout).packages) for (const target of pkg.targets) map.set(target.name.replaceAll('-', '_'), pkg.name);
	return map;
}

/** One entry per test binary: { crate, target, passed, failed, ignored, seconds, failures: [names] }. */
export function parseRuns(output, packages = new Map()) {
	const packageOf = (target) => packages.get(target.replaceAll('-', '_')) || target.replaceAll('_', '-');
	const runs = [];
	let crate = '?';
	let current = null;
	for (const line of output.split('\n')) {
		const unit = line.match(/^\s+Running unittests (\S+) \(.*?\/deps\/([\w-]+?)-[0-9a-f]+\)/);
		const integration = line.match(/^\s+Running ((?:tests|benches|examples)\/\S+)/);
		const doc = line.match(/^\s+Doc-tests (\S+)/);
		if (unit) {
			crate = packageOf(unit[2]);
			current = { crate, target: `unit tests (${unit[1]})` };
		} else if (integration) {
			// Cargo runs a crate's unit tests first, then its integration tests: they belong to it.
			current = { crate, target: integration[1] };
		} else if (doc) {
			crate = packageOf(doc[1]);
			current = { crate, target: 'doc tests' };
		}
		const failure = line.match(/^---- (\S+) stdout ----$/);
		if (failure && current) (current.failures ||= []).push(failure[1]);
		const done = line.match(/^test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored; \d+ measured; \d+ filtered out; finished in ([\d.]+)s/);
		if (done && current) {
			const [, passed, failed, ignored, seconds] = done.map(Number);
			runs.push({ failures: [], ...current, passed, failed, ignored, seconds });
			current = { crate, target: current.target };
		}
	}
	return runs;
}

/** `backend/crates/gitprompt-core/src/x.rs` → gitprompt-core; apps/desktop → gitprompt-desktop. */
function crateOf(file) {
	const match = file.match(/backend\/(crates|apps)\/([^/]+)\//);
	if (!match) return null;
	return match[1] === 'apps' ? `gitprompt-${match[2]}` : match[2];
}

function page(ctx, runs, coverage) {
	const sum = (key) => runs.reduce((total, run) => total + run[key], 0);
	const [passed, failed, ignored] = [sum('passed'), sum('failed'), sum('ignored')];
	const crates = [...new Set(runs.map((run) => run.crate))];
	const files = (coverage?.data[0].files || []).map((file) => ({
		path: file.filename.replace(`${ctx.repo}/`, ''),
		crate: crateOf(file.filename),
		lines: file.summary.lines
	}));
	const totals = coverage?.data[0].totals;
	const lineShare = totals ? totals.lines.covered / totals.lines.count : null;
	const coverageOf = (crate) => {
		const mine = files.filter((file) => file.crate === crate);
		return { covered: mine.reduce((t, f) => t + f.lines.covered, 0), count: mine.reduce((t, f) => t + f.lines.count, 0) };
	};

	const sections = [
		kpis([
			{ label: 'Tests passed', value: passed, tone: failed ? 'bad' : 'good' },
			{ label: 'Failed', value: failed, tone: failed ? 'bad' : 'good' },
			{ label: 'Ignored', value: ignored, tone: ignored ? 'warn' : 'neutral' },
			{ label: 'Test binaries', value: runs.length },
			{ label: 'Line coverage', value: pct(lineShare), tone: tone(lineShare, 0.8, 0.6) },
			{ label: 'Function coverage', value: totals ? pct(totals.functions.covered / totals.functions.count) : '–' }
		])
	];
	if (failed) {
		sections.push(note(`${plural(failed, 'test')} failed, so coverage was not measured this run. CI fails too: fix them first.`, 'bad'));
		const failures = runs.flatMap((run) => run.failures.map((name) => ({ crate: run.crate, target: run.target, test: name })));
		sections.push(table('Failing tests', [{ key: 'crate', label: 'Crate' }, { key: 'target', label: 'Binary' }, { key: 'test', label: 'Test', mono: true }], failures));
	}
	if (coverage) {
		sections.push(
			bars(
				'Line coverage by crate',
				crates.map((crate) => {
					const c = coverageOf(crate);
					return { label: crate, value: c.count ? c.covered / c.count : 0, text: c.count ? `${pct(c.covered / c.count)} · ${c.covered}/${c.count}` : 'not measured', tone: c.count ? tone(c.covered / c.count, 0.8, 0.6) : 'neutral' };
				}),
				'Left out: test code, examples and main.rs. A crate tested mostly by running its binary in a child process may read low.'
			)
		);
	}
	sections.push(
		table(
			'Tests by crate',
			[
				{ key: 'crate', label: 'Crate', mono: true },
				{ key: 'binaries', label: 'Binaries', align: 'right' },
				{ key: 'passed', label: 'Passed', align: 'right' },
				{ key: 'failed', label: 'Failed', align: 'right' },
				{ key: 'ignored', label: 'Ignored', align: 'right' },
				{ key: 'seconds', label: 'Time', align: 'right' },
				...(coverage ? [{ key: 'coverage', label: 'Coverage', align: 'right' }] : [])
			],
			crates.map((crate) => {
				const mine = runs.filter((run) => run.crate === crate);
				const of = (key) => mine.reduce((t, run) => t + run[key], 0);
				const c = coverageOf(crate);
				return {
					crate,
					binaries: mine.length,
					passed: of('passed'),
					failed: { text: of('failed'), tone: of('failed') ? 'bad' : undefined, sort: of('failed') },
					ignored: of('ignored'),
					seconds: { text: `${of('seconds').toFixed(2)} s`, sort: of('seconds') },
					coverage: coverageCell(c.covered, c.count)
				};
			})
		),
		table(
			'Test binaries',
			[
				{ key: 'crate', label: 'Crate', mono: true },
				{ key: 'target', label: 'Binary', mono: true },
				{ key: 'passed', label: 'Passed', align: 'right' },
				{ key: 'failed', label: 'Failed', align: 'right' },
				{ key: 'seconds', label: 'Time', align: 'right' }
			],
			runs.map((run) => ({ ...run, failed: { text: run.failed, tone: run.failed ? 'bad' : undefined, sort: run.failed }, seconds: { text: `${run.seconds.toFixed(2)} s`, sort: run.seconds } }))
		)
	);
	if (coverage) {
		const weakest = files.filter((file) => file.lines.count >= 10).sort((a, b) => a.lines.covered / a.lines.count - b.lines.covered / b.lines.count);
		sections.push(
			table(
				'Files by coverage, least covered first',
				[
					{ key: 'path', label: 'File', mono: true },
					{ key: 'lines', label: 'Lines', align: 'right' },
					{ key: 'coverage', label: 'Coverage', align: 'right' }
				],
				weakest.map((file) => ({ path: file.path, lines: file.lines.count, coverage: coverageCell(file.lines.covered, file.lines.count) })),
				'Files with at least 10 coverable lines.'
			),
			embed('Line by line (cargo llvm-cov)', ctx.fromHub(join(ctx.backend, 'target/llvm-cov/html/index.html')))
		);
	}
	return {
		headline: failed ? { value: failed, label: failed === 1 ? 'test failing' : 'tests failing', tone: 'bad' } : { value: pct(lineShare), label: 'lines covered', tone: tone(lineShare, 0.8, 0.6) },
		summary: `${plural(passed, 'test')} passed${failed ? `, ${failed} failed` : ''} in ${plural(runs.length, 'binary', 'binaries')}${totals ? ` · ${totals.lines.covered} of ${totals.lines.count} lines covered` : ''}`,
		sections
	};
}
