// Build steps several reports need, each done once per build (ctx.once): SvelteKit's generated
// types, the web build, clippy, the daemon (debug and release) and desktop app builds, the demo data
// (as spooled, and drained) and a daemon with it to measure the dashboard against. Reports run side by side, so whatever writes into web/
// or backend/target for several of them belongs here.
import { cp, mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { profileVars } from '../../env.mjs';
import { installed } from './context.mjs';
import { cargoActivity } from './output.mjs';
import { startDaemon } from './process.mjs';

/**
 * web/.svelte-kit (the types and tsconfig every web tool reads), written once before any of them
 * runs: later syncs (vite build runs one) only write what changed, so nothing is rewritten while
 * another report reads it.
 */
export function webSync(ctx) {
	return ctx.once('the SvelteKit sync', () => ctx.exec('pnpm', ['--dir', 'web', 'exec', 'svelte-kit', 'sync'], { label: 'svelte-kit sync' }));
}

/** web/build, built once per build. */
export function webBuild(ctx) {
	return ctx.once('the web build', async () => {
		await webSync(ctx);
		return ctx.exec('pnpm', ['--dir', 'web', 'build'], { label: 'building the web dashboard (vite build)' });
	});
}

/** `cargo clippy` on the whole workspace as JSON, once per build (the lint and complexity reports read it). */
export function clippy(ctx) {
	return ctx.once('clippy', () =>
		ctx.exec('cargo', ['clippy', '--workspace', '--all-targets', '--message-format=json'], {
			cwd: ctx.backend,
			label: 'cargo clippy --workspace --all-targets',
			allowFailure: true,
			activity: cargoActivity(),
			dataOnStdout: true
		})
	);
}

/** backend/target/debug/gitprompt, built once per build. */
export function daemonBinary(ctx) {
	return ctx.once('the daemon build', async () => {
		await ctx.exec('cargo', ['build', '-p', 'gitprompt-cli'], { cwd: ctx.backend, label: 'building the daemon (cargo build)', activity: cargoActivity() });
		return join(ctx.backend, 'target', 'debug', process.platform === 'win32' ? 'gitprompt.exe' : 'gitprompt');
	});
}

/** backend/target/release/gitprompt, built once per build: what timings measure (a debug build is several times slower). */
export function releaseDaemonBinary(ctx) {
	return ctx.once('the release daemon build', async () => {
		await ctx.exec('cargo', ['build', '--release', '-p', 'gitprompt-cli'], { cwd: ctx.backend, label: 'building the daemon (cargo build --release)', activity: cargoActivity() });
		return join(ctx.backend, 'target', 'release', process.platform === 'win32' ? 'gitprompt.exe' : 'gitprompt');
	});
}

/**
 * A data folder with the demo prompts of tools/seed-demo.py in its spool, written once per build and
 * never served: each daemon gets its own copy (demoHome), since a daemon drains the spool it starts on.
 */
function demoSeed(ctx) {
	return ctx.once('the demo prompts', async () => {
		const home = await tempFolder(ctx);
		const seed = installed('uv') ? ['uv', ['run', '--no-project', 'python', 'tools/seed-demo.py', home]] : ['python3', ['tools/seed-demo.py', home]];
		await ctx.exec(seed[0], seed[1], { label: 'writing demo prompts (tools/seed-demo.py)' });
		return home;
	});
}

/** A fresh copy of the demo data folder (never ~/.gitprompt), removed after the build. */
export async function demoHome(ctx) {
	return copyOf(ctx, await demoSeed(ctx));
}

/**
 * The demo data with its spool already drained into the database, once per build (a start on it
 * then measures the start, not the drain): copy it with copyOf before starting on it.
 */
export function drainedDemo(ctx) {
	return ctx.once('the drained demo data', async () => {
		const [binary, home] = await Promise.all([releaseDaemonBinary(ctx), demoHome(ctx)]);
		ctx.step('draining the demo spool into a database (release daemon)');
		const daemon = await startDaemon(binary, { home, dashboard: join(ctx.web, 'build'), env: performanceEnv() });
		try {
			await daemon.drained();
		} finally {
			await daemon.stop();
		}
		return home;
	});
}

/** A temporary copy of a folder, removed after the build. */
export async function copyOf(ctx, folder) {
	const copy = await tempFolder(ctx);
	await cp(folder, copy, { recursive: true });
	return copy;
}

/** The variables of the performance profile (.env.performance): what every timed process runs with. */
export const performanceEnv = () => profileVars('performance');

/** backend/target/release/gitprompt-desktop, built once per build (no installer, no bundling). */
export function desktopBinary(ctx) {
	return ctx.once('the release desktop build', async () => {
		await ctx.exec('cargo', ['build', '--release', '-p', 'gitprompt-desktop'], { cwd: ctx.backend, label: 'building the desktop app (cargo build --release)', activity: cargoActivity() });
		return join(ctx.backend, 'target', 'release', process.platform === 'win32' ? 'gitprompt-desktop.exe' : 'gitprompt-desktop');
	});
}

/**
 * A debug daemon on a free loopback port serving the fresh web/build, on a copy of the demo data.
 * Stopped after the build. Resolves { url }.
 */
export function demoDaemon(ctx) {
	return ctx.once('the demo daemon', async () => {
		await webBuild(ctx);
		const binary = await daemonBinary(ctx);
		const home = await demoHome(ctx);
		ctx.step('starting a demo daemon and waiting for it to drain the demo spool');
		const daemon = await startDaemon(binary, { home, dashboard: join(ctx.web, 'build') });
		ctx.cleanup(daemon.stop);
		// Pages measured on half the data would measure a moving target. Setup, not a measurement: a
		// debug build draining thousands of payloads while other reports build takes a while.
		await daemon.drained();
		return { url: daemon.url };
	});
}

/** A new empty temporary folder, removed after the build. */
export async function tempFolder(ctx) {
	const folder = await mkdtemp(join(tmpdir(), 'gitprompt-quality-'));
	ctx.cleanup(() => rm(folder, { recursive: true, force: true }));
	return folder;
}
