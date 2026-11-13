// The repository's environment profiles: GITPROMPT_ENV and the variables that go with it, from
// .env files at the repository root, layered as Vite does (later wins):
//   .env  →  .env.local  →  .env.<profile>  →  .env.<profile>.local
// .env and .env.<profile> are committed; the .local files are yours (git-ignored). A variable already
// set in the shell wins over every file. Profiles: development, production, performance.
//
//   node tools/env.mjs development -- cargo run -p gitprompt-desktop   run a command in a profile
//   eval "$(node tools/env.mjs development --export)"                  load it into a bash recipe
//
// The binaries never read .env files: an installed app runs in production unless GITPROMPT_ENV
// says otherwise. Only the recipes and the quality reports load them.
import { spawn } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseEnv } from 'node:util';

export const PROFILES = ['development', 'production', 'performance'];
const ROOT = dirname(dirname(fileURLToPath(import.meta.url)));

/** The files of a profile, in the order they apply. */
export const profileFiles = (profile) => ['.env', '.env.local', `.env.${profile}`, `.env.${profile}.local`];

/** The variables the files of `profile` set (without the shell's); throws on an unknown profile. */
export function profileVars(profile, root = ROOT) {
	if (!PROFILES.includes(profile)) throw new Error(`unknown profile ${profile}: expected ${PROFILES.join(', ')}`);
	const vars = {};
	for (const file of profileFiles(profile)) Object.assign(vars, readVars(join(root, file)));
	return vars;
}

function readVars(path) {
	try {
		return parseEnv(readFileSync(path, 'utf8'));
	} catch (error) {
		if (error.code === 'ENOENT') return {};
		throw error;
	}
}

/** The environment a command of `profile` runs with: the files, then the shell over them. */
export const profileEnv = (profile, shell = process.env) => ({ ...profileVars(profile), ...shell });

/** `export NAME='value'` lines for bash, for the variables the shell does not set already. */
export function exportLines(vars, shell = process.env) {
	return Object.entries(vars)
		.filter(([name]) => shell[name] === undefined)
		.map(([name, value]) => `export ${name}='${value.replaceAll("'", "'\\''")}'`)
		.join('\n');
}

function main([profile, mode, ...command]) {
	if (mode === '--export') {
		const lines = exportLines(profileVars(profile));
		if (lines) process.stdout.write(`${lines}\n`);
		return;
	}
	if (mode !== '--' || command.length === 0) {
		process.stderr.write('usage: node tools/env.mjs <profile> -- <command> [args...]\n       node tools/env.mjs <profile> --export\n');
		process.exit(2);
	}
	const child = spawn(command[0], command.slice(1), { env: profileEnv(profile), stdio: 'inherit', shell: process.platform === 'win32' });
	// Ctrl-C reaches the command from the terminal itself; only pass on what was sent to this process.
	process.on('SIGINT', () => {});
	process.on('SIGTERM', () => child.kill('SIGTERM'));
	child.on('error', (error) => {
		process.stderr.write(`${command[0]}: ${error.message}\n`);
		process.exit(127);
	});
	child.on('exit', (code, signal) => process.exit(code ?? (signal ? 1 : 0)));
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
	try {
		main(process.argv.slice(2));
	} catch (error) {
		process.stderr.write(`tools/env.mjs: ${error.message}\n`);
		process.exit(2);
	}
}
