#!/usr/bin/env node
// Flaky tests: the suites run again and again, several at once so they load the machine the way a
// busy CI runner does, and every test that failed even once is named. The safety net for what the
// time-dependency rules cannot see in the text (races, reused ports, ordering): `just flaky`.
//
//   node tools/timing/flaky.mjs                         the Rust workspace, 8 runs, 4 at a time
//   node tools/timing/flaky.mjs --runs 20 --jobs 6      more runs, more load
//   node tools/timing/flaky.mjs --web                   the dashboard's tests too
//   node tools/timing/flaky.mjs -- -p gitprompt-cli     cargo test arguments (a crate, a test)
//
// Exit 1 when any run failed. A failed run's output is kept in target/flaky/.
import { spawn } from 'node:child_process';
import { mkdir, rm, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const REPO = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const LOGS = join(REPO, 'target', 'flaky');

/** { runs, jobs, web, cargo: [args] } from the command line. */
export function options(argv) {
	const split = argv.indexOf('--');
	const own = split === -1 ? argv : argv.slice(0, split);
	const value = (name, fallback) => {
		const at = own.indexOf(name);
		const n = at === -1 ? fallback : Number(own[at + 1]);
		if (!Number.isInteger(n) || n < 1) throw new Error(`${name} takes a whole number of 1 or more`);
		return n;
	};
	return { runs: value('--runs', 8), jobs: value('--jobs', 4), web: own.includes('--web'), cargo: split === -1 ? ['--workspace'] : argv.slice(split + 1) };
}

/** The tests a run's output says failed: cargo's `---- name stdout ----`, vitest's `FAIL  file > name`. */
export function failedTests(output) {
	const cargo = [...output.matchAll(/^---- (.+?) stdout ----$/gm)].map((m) => m[1]);
	const vitest = [...output.matchAll(/^\s*(?:×|FAIL)\s+(\S.*?)(?:\s+\d+ms)?$/gm)].map((m) => m[1].trim());
	return [...new Set([...cargo, ...vitest])];
}

/** Run a command to its end: { code, output } (stdout and stderr together). */
function run(command, args, cwd) {
	return new Promise((resolve, reject) => {
		const child = spawn(command, args, { cwd, stdio: ['ignore', 'pipe', 'pipe'] });
		let output = '';
		const keep = (chunk) => (output += chunk);
		child.stdout.on('data', keep);
		child.stderr.on('data', keep);
		child.once('error', reject);
		child.once('exit', (code) => resolve({ code: code ?? 1, output }));
	});
}

/** `count` runs of `suite`, `jobs` at a time: [{ suite, index, code, output }]. */
async function repeat(suite, count, jobs) {
	const results = [];
	let next = 0;
	const worker = async () => {
		while (next < count) {
			const index = next++;
			results.push({ suite: suite.name, index, ...(await run(suite.command, await suite.args(index), suite.cwd)) });
		}
	};
	await Promise.all(Array.from({ length: Math.min(jobs, count) }, worker));
	return results;
}

/**
 * The vitest arguments of web run `index`: the dashboard's own config with its caches in a folder
 * of this run. Runs at once would otherwise share Vite's dependency cache (node_modules/.vite) and
 * break each other renaming it: a failure of the tool, not of a test.
 */
async function vitestArgs(index) {
	const dir = join(LOGS, `web-${index + 1}`);
	await mkdir(dir, { recursive: true });
	const config = join(dir, 'vitest.config.mjs');
	const base = pathToFileURL(join(REPO, 'web', 'vite.config.ts')).href;
	const settings = { root: join(REPO, 'web'), cacheDir: join(dir, 'vite') };
	await writeFile(config, `import base from ${JSON.stringify(base)};\nexport default { ...base, ...${JSON.stringify(settings)} };\n`);
	return ['exec', 'vitest', 'run', '--config', config, '--fsModuleCachePath', join(dir, 'modules')];
}

async function main() {
	const { runs, jobs, web, cargo } = options(process.argv.slice(2));
	const backend = join(REPO, 'backend');
	const suites = [{ name: 'rust', command: 'cargo', args: () => ['test', '-q', ...cargo], cwd: backend }];
	if (web) suites.push({ name: 'web', command: 'pnpm', args: vitestArgs, cwd: join(REPO, 'web') });
	console.log(`Building the Rust tests once (cargo test --no-run ${cargo.join(' ')})…`);
	const built = await run('cargo', ['test', '--no-run', ...cargo], backend);
	if (built.code !== 0) {
		console.error(built.output.slice(-4000));
		throw new Error('the tests did not build');
	}
	await rm(LOGS, { recursive: true, force: true });
	await mkdir(LOGS, { recursive: true });
	const results = [];
	for (const suite of suites) {
		console.log(`Running ${suite.name}: ${runs} times, ${jobs} at once…`);
		results.push(...(await repeat(suite, runs, jobs)));
	}
	return report(results);
}

/** Print what failed and keep each failed run's output; the exit code. */
async function report(results) {
	const failed = results.filter((r) => r.code !== 0);
	const counts = new Map();
	for (const r of failed) {
		await writeFile(join(LOGS, `${r.suite}-${r.index + 1}.log`), r.output);
		const tests = failedTests(r.output);
		for (const test of tests.length ? tests : ['(no test named: see the log)']) counts.set(`${r.suite}: ${test}`, (counts.get(`${r.suite}: ${test}`) ?? 0) + 1);
	}
	for (const name of new Set(results.map((r) => r.suite))) {
		const mine = results.filter((r) => r.suite === name);
		console.log(`${name}: ${mine.length - mine.filter((r) => r.code !== 0).length} of ${mine.length} runs passed`);
	}
	if (!failed.length) return 0;
	console.log('\nFailed, in how many runs:');
	for (const [test, n] of [...counts].sort((a, b) => b[1] - a[1])) console.log(`  ${n}× ${test}`);
	console.log(`\nLogs of the failed runs: ${LOGS}`);
	return 1;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
	main().then(
		(code) => (process.exitCode = code),
		(error) => {
			console.error(`flaky: ${error.message}`);
			process.exitCode = 2;
		}
	);
}
