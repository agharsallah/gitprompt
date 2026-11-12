// Build steps several reports need, each done once per build (ctx.once): SvelteKit's generated
// types, the web build, clippy, the daemon binary, and a daemon with demo data to measure the
// dashboard against. Reports run side by side, so whatever writes into web/ or backend/target for
// several of them belongs here.
import { spawn } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import { createServer } from 'node:net';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { installed } from './context.mjs';
import { cargoActivity } from './output.mjs';

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

/**
 * A daemon on a free loopback port serving the fresh web/build, with the demo prompts of
 * tools/seed-demo.py in a temporary data folder (never ~/.gitprompt). Stopped after the build.
 * Resolves { url }.
 */
export function demoDaemon(ctx) {
	return ctx.once('the demo daemon', async () => {
		await webBuild(ctx);
		const binary = await daemonBinary(ctx);
		const home = await mkdtemp(join(tmpdir(), 'gitprompt-quality-'));
		ctx.cleanup(() => rm(home, { recursive: true, force: true }));
		const seed = installed('uv') ? ['uv', ['run', '--no-project', 'python', 'tools/seed-demo.py', home]] : ['python3', ['tools/seed-demo.py', home]];
		await ctx.exec(seed[0], seed[1], { label: 'writing demo prompts (tools/seed-demo.py)' });
		const port = await freePort();
		ctx.step(`starting a demo daemon on :${port}`);
		const env = { ...process.env, GITPROMPT_HOME: home, GITPROMPT_PORT: String(port), GITPROMPT_DASHBOARD: join(ctx.web, 'build') };
		const daemon = spawn(binary, ['serve'], { env, stdio: 'ignore' });
		ctx.cleanup(() => daemon.kill());
		const url = `http://127.0.0.1:${port}`;
		await ready(`${url}/api/status`, 60_000);
		return { url };
	});
}

function freePort() {
	return new Promise((resolve, reject) => {
		const server = createServer();
		server.on('error', reject);
		server.listen(0, '127.0.0.1', () => {
			const { port } = server.address();
			server.close(() => resolve(port));
		});
	});
}

async function ready(url, timeoutMs) {
	const until = Date.now() + timeoutMs;
	while (Date.now() < until) {
		// One attempt never waits long: a request the daemon took while starting may never be answered.
		const ok = await fetch(url, { signal: AbortSignal.timeout(2_000) }).then(
			(response) => response.ok,
			() => false
		);
		if (ok) return;
		await new Promise((resolve) => setTimeout(resolve, 300));
	}
	throw new Error(`the demo daemon did not answer ${url} within ${timeoutMs / 1000} s`);
}
