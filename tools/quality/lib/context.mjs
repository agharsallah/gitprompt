// What a report's run(ctx) gets: run commands (logged, with a live step label and their latest
// output), read files, share one-off steps with other reports in the same build, measure alone,
// and clean up after the build.
import { spawn, spawnSync } from 'node:child_process';
import { createWriteStream } from 'node:fs';
import { mkdir, readFile, stat } from 'node:fs/promises';
import { join } from 'node:path';

import { lineSplitter } from './output.mjs';
import { PATHS, REPO, fromHub } from './paths.mjs';

const WINDOWS = process.platform === 'win32';
/** Lines of output kept for the error message of a failed command. */
const TAIL = 25;

export class CommandError extends Error {}

/**
 * build: what the reports of one build share: { shared: Map, owners: Map, cleanups: [], gate }
 * (scheduler.mjs's gate). slots: the scheduler's semaphore, handed back while measuring alone.
 * live: { step(text), activity(text), line(text), waiting(text | null), measuring(flag) }, what the
 * build shows (measuring: this report waits for, or holds, the machine alone).
 */
export async function createContext(report, { build, slots, live }) {
	const artifacts = join(PATHS.artifacts, report.area, report.name);
	await mkdir(artifacts, { recursive: true });
	await mkdir(PATHS.logs, { recursive: true });
	const logPath = join(PATHS.logs, `${report.area}-${report.name}.log`);
	const log = createWriteStream(logPath);
	log.write(`# ${report.id} · ${new Date().toISOString()}\n`);
	/** Labels of the commands running now (a report may run several at once). */
	const running = [];
	let alone = false;
	let waitedMs = 0;

	const waitFor = async (text, promise) => {
		const since = Date.now();
		live.waiting(text);
		try {
			return await promise;
		} finally {
			live.waiting(null);
			waitedMs += Date.now() - since;
		}
	};

	const showLine = (activity) => (line) => {
		live.line(line);
		const text = activity ? activity(line) : line;
		if (text !== undefined) live.activity(text);
	};

	const spawnLogged = (command, args, { cwd, env, label, allowFailure, activity, dataOnStdout }) =>
		new Promise((resolve, reject) => {
			const child = spawn(command, args, { cwd, env: { ...process.env, ...env }, shell: WINDOWS });
			const out = [];
			const err = [];
			const both = [];
			const lines = { stdout: lineSplitter(dataOnStdout ? () => {} : showLine(activity)), stderr: lineSplitter(showLine(activity)) };
			const collect = (target, stream) => (chunk) => {
				target.push(chunk);
				both.push(chunk);
				log.write(chunk);
				lines[stream].push(chunk);
			};
			child.stdout.on('data', collect(out, 'stdout'));
			child.stderr.on('data', collect(err, 'stderr'));
			child.on('error', (error) => reject(new CommandError(`${command}: ${error.message}`)));
			child.on('close', (code) => {
				lines.stdout.end();
				lines.stderr.end();
				const result = { code: code ?? 1, stdout: Buffer.concat(out).toString(), stderr: Buffer.concat(err).toString(), output: Buffer.concat(both).toString() };
				log.write(`\n(exit ${result.code})\n`);
				if (result.code === 0 || allowFailure) resolve(result);
				else reject(new CommandError(`${label || command} failed (exit ${result.code})\n${tail(result)}`));
			});
		});

	const ctx = {
		repo: REPO,
		backend: join(REPO, 'backend'),
		web: join(REPO, 'web'),
		/** This report's own folder for files it writes (HTML reports, JSON). */
		artifacts,
		logPath,
		fromHub,
		/** Time spent waiting for a measurement slot or another report's shared step. */
		get waitedMs() {
			return waitedMs;
		},
		step(text) {
			log.write(`\n## ${text}\n`);
			live.step(text);
		},
		log(text) {
			log.write(`${text}\n`);
		},
		/**
		 * Runs a command; resolves { code, stdout, stderr, output } (output: both streams in the
		 * order they arrived, as a terminal shows them). Rejects on a non-zero exit unless
		 * allowFailure. Output goes to the report's log and, line by line, to the live view;
		 * activity(line) → text | undefined turns a line into what the view says (output.mjs's
		 * cargoActivity), else the view shows the latest line; dataOnStdout keeps stdout (JSON a report
		 * parses) out of the view. Waits while a measurement runs alone.
		 */
		async exec(command, args = [], { cwd = REPO, env = {}, label, allowFailure = false, activity, dataOnStdout = false } = {}) {
			const name = label || `${command} ${args.join(' ')}`.slice(0, 90);
			const run = async () => {
				running.push(name);
				ctx.step(running.join(' + '));
				live.activity('');
				log.write(`$ ${command} ${args.join(' ')}\n`);
				try {
					return await spawnLogged(command, args, { cwd, env, label, allowFailure, activity, dataOnStdout });
				} finally {
					running.splice(running.indexOf(name), 1);
					if (running.length > 0) live.step(running.join(' + '));
				}
			};
			if (alone) return run();
			if (!build.gate.measuring()) return build.gate.shared(run);
			const since = Date.now();
			live.waiting('paused while another report measures alone');
			return build.gate.shared(() => {
				waitedMs += Date.now() - since;
				live.waiting(null);
				return run();
			});
		},
		/**
		 * Runs fn once per build under `key`, however many reports ask: a shared build step. A report
		 * asking while another one runs it waits for it, and says so.
		 */
		once(key, fn) {
			if (!build.shared.has(key)) {
				build.owners.set(key, report.id);
				build.shared.set(key, fn());
				return build.shared.get(key);
			}
			const owner = build.owners.get(key);
			const promise = build.shared.get(key);
			if (owner === report.id) return promise;
			return waitFor(`waiting for ${key} (run by ${owner})`, promise);
		},
		/**
		 * Runs fn once every other report finished (or waits to measure too), with no command running:
		 * for measurements a busy machine would skew. The report's place is free meanwhile. Build what
		 * fn needs before: a shared step started by another report could not run meanwhile.
		 */
		async alone(reason, fn) {
			slots.release();
			const since = Date.now();
			live.waiting(`built; measures ${reason} alone once the other reports are done`);
			live.measuring(true);
			try {
				return await build.gate.alone(async () => {
					waitedMs += Date.now() - since;
					live.waiting(null);
					alone = true;
					try {
						return await fn();
					} finally {
						alone = false;
					}
				});
			} finally {
				live.waiting(null);
				live.measuring(false);
				await slots.acquire();
			}
		},
		/** Runs after the whole build, even when it fails (stop a server, remove a temp folder). */
		cleanup(fn) {
			build.cleanups.push(fn);
		},
		async readJson(path) {
			return JSON.parse(await readFile(path, 'utf8'));
		},
		async exists(path) {
			return stat(path).then(
				() => true,
				() => false
			);
		},
		close() {
			return new Promise((resolve) => log.end(resolve));
		}
	};
	return ctx;
}

/** The commands of `needs` that are not installed: [{ command, install }]. */
export function missing(needs) {
	return needs.filter((need) => !installed(need.command));
}

export function installed(command) {
	const probe = WINDOWS ? spawnSync('where', [command]) : spawnSync('sh', ['-c', `command -v ${command}`]);
	return probe.status === 0;
}

function tail({ stdout, stderr }) {
	const lines = `${stdout}\n${stderr}`.split('\n').filter((line) => line.trim());
	return lines.slice(-TAIL).join('\n');
}
