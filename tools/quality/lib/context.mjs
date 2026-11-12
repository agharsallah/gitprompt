// What a report's run(ctx) gets: run commands (logged, with a live step label), read files, share
// one-off steps with other reports in the same build, and clean up after the build.
import { spawn, spawnSync } from 'node:child_process';
import { createWriteStream } from 'node:fs';
import { mkdir, readFile, stat } from 'node:fs/promises';
import { join } from 'node:path';

import { PATHS, REPO, fromHub } from './paths.mjs';

const WINDOWS = process.platform === 'win32';
/** Lines of output kept for the error message of a failed command. */
const TAIL = 25;

export class CommandError extends Error {}

/**
 * build: { shared: Map, cleanups: [] } for the whole run; onStep(text) shows what the report is doing.
 */
export async function createContext(report, build, onStep) {
	const artifacts = join(PATHS.artifacts, report.area, report.name);
	await mkdir(artifacts, { recursive: true });
	await mkdir(PATHS.logs, { recursive: true });
	const logPath = join(PATHS.logs, `${report.area}-${report.name}.log`);
	const log = createWriteStream(logPath);
	log.write(`# ${report.id} · ${new Date().toISOString()}\n`);

	const ctx = {
		repo: REPO,
		backend: join(REPO, 'backend'),
		web: join(REPO, 'web'),
		/** This report's own folder for files it writes (HTML reports, JSON). */
		artifacts,
		logPath,
		fromHub,
		step(text) {
			log.write(`\n## ${text}\n`);
			onStep(text);
		},
		log(text) {
			log.write(`${text}\n`);
		},
		/**
		 * Runs a command; resolves { code, stdout, stderr, output } (output: both streams in the
		 * order they arrived, as a terminal shows them). Rejects on a non-zero exit unless
		 * allowFailure. Output goes to the report's log, not the terminal.
		 */
		exec(command, args = [], { cwd = REPO, env = {}, label, allowFailure = false } = {}) {
			ctx.step(label || `${command} ${args.join(' ')}`.slice(0, 90));
			log.write(`$ ${command} ${args.join(' ')}\n`);
			return new Promise((resolve, reject) => {
				const child = spawn(command, args, { cwd, env: { ...process.env, ...env }, shell: WINDOWS });
				const out = [];
				const err = [];
				const both = [];
				child.stdout.on('data', (chunk) => {
					out.push(chunk);
					both.push(chunk);
					log.write(chunk);
				});
				child.stderr.on('data', (chunk) => {
					err.push(chunk);
					both.push(chunk);
					log.write(chunk);
				});
				child.on('error', (error) => reject(new CommandError(`${command}: ${error.message}`)));
				child.on('close', (code) => {
					const result = { code: code ?? 1, stdout: Buffer.concat(out).toString(), stderr: Buffer.concat(err).toString(), output: Buffer.concat(both).toString() };
					log.write(`\n(exit ${result.code})\n`);
					if (result.code === 0 || allowFailure) resolve(result);
					else reject(new CommandError(`${label || command} failed (exit ${result.code})\n${tail(result)}`));
				});
			});
		},
		/** Runs fn once per build under `key`, however many reports ask: a shared build step. */
		once(key, fn) {
			if (!build.shared.has(key)) build.shared.set(key, fn());
			return build.shared.get(key);
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
