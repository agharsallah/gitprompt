// Every function's cognitive complexity and length, measured on the syntax tree by the architecture
// test the gate runs (complexity.rs; COMPLEXITY_JSON makes it write each function's values); file
// sizes against MAX_LINES; clippy's other limits (nesting, arguments, lines) as findings; and the
// functions kept complex on purpose (COMPLEX in rules.rs, #[expect(clippy::…, reason)]).
import { readFile, readdir } from 'node:fs/promises';
import { join, relative } from 'node:path';

import { complexityPage } from '../../lib/complexity.mjs';
import { parseClippy } from './30-lint.mjs';

const RULES = 'crates/gitprompt-core/tests/architecture/rules.rs';
const CLIPPY_LIMITS = ['clippy::excessive_nesting', 'clippy::too_many_arguments', 'clippy::too_many_lines'];

export default {
	title: 'Complexity',
	description: 'Cognitive complexity and length of every function, file sizes, clippy’s nesting and argument limits, and the functions kept complex on purpose.',
	async run(ctx) {
		const rules = await readFile(join(ctx.backend, RULES), 'utf8');
		const clippyToml = await readFile(join(ctx.backend, 'clippy.toml'), 'utf8');
		const out = join(ctx.artifacts, 'functions.json');
		await ctx.exec('cargo', ['test', '-q', '-p', 'gitprompt-core', '--test', 'architecture', 'complexity_report'], {
			cwd: ctx.backend,
			env: { COMPLEXITY_JSON: out },
			label: 'measure every function (architecture test)'
		});
		const clippy = await ctx.exec('cargo', ['clippy', '--workspace', '--all-targets', '--message-format=json', '-q'], {
			cwd: ctx.backend,
			label: 'cargo clippy (nesting, arguments, lines)',
			allowFailure: true
		});
		const findings = parseClippy(clippy.stdout).filter((f) => CLIPPY_LIMITS.includes(f.lint));
		const sources = await rustSources(ctx.backend);
		const fileLimits = maxLines(rules);
		return complexityPage({
			functions: (await ctx.readJson(out)).map((f) => ({ ...f, place: `${f.file}:${f.line}`, unit: f.file.split('/')[1] })),
			files: await Promise.all(sources.map(async ({ file, tests }) => ({ file, lines: await lineCount(join(ctx.backend, file)), limit: tests ? fileLimits.tests : fileLimits.src }))),
			limits: { complexity: constant(rules, 'MAX_COMPLEXITY'), lines: setting(clippyToml, 'too-many-lines-threshold') },
			findings: findings.map((f) => ({ place: `${relative(ctx.backend, join(ctx.backend, f.file))}:${f.line}`, rule: f.lint.slice(8), message: f.message })),
			exemptions: [...complexEntries(rules), ...(await clippyExemptions(ctx.backend, sources))],
			measure:
				'Complexity is cognitive complexity (SonarSource) on the syntax tree: each if, match, loop or let-else costs 1 plus its nesting depth, else and boolean-operator runs 1; `?` and macros (logging) cost nothing. Lines are code lines, without blanks or comments.'
		});
	}
};

function constant(rust, name) {
	const match = rust.match(new RegExp(`const ${name}: usize = (\\d+)`));
	if (!match) throw new Error(`no ${name} in ${RULES}`);
	return Number(match[1]);
}

function setting(toml, key) {
	const match = toml.match(new RegExp(`^${key}\\s*=\\s*(\\d+)`, 'm'));
	if (!match) throw new Error(`clippy.toml sets no ${key}`);
	return Number(match[1]);
}

function maxLines(rules) {
	const match = rules.match(/MAX_LINES:\s*\(usize, usize\)\s*=\s*\((\d+),\s*(\d+)\)/);
	if (!match) throw new Error(`no MAX_LINES in ${RULES}`);
	return { src: Number(match[1]), tests: Number(match[2]) };
}

/** `("crates/x/src/y.rs::name", "why")` entries of COMPLEX. */
function complexEntries(rules) {
	const list = rules.match(/pub const COMPLEX[^=]*=\s*&\[([\s\S]*?)\];/)?.[1] ?? '';
	return [...list.matchAll(/\(\s*"([^"]+)::(\w+)",\s*"((?:[^"\\]|\\.)*)"\s*\)/g)].map((m) => ({ place: `${m[1]} ${m[2]}`, rule: 'cognitive complexity', reason: m[3] }));
}

/** Every .rs file under a crate's src/ and tests/ (examples and benches too, against the src limit). */
async function rustSources(backend) {
	const files = [];
	for (const entry of await readdir(join(backend, 'crates'), { recursive: true, withFileTypes: true })) {
		if (!entry.isFile() || !entry.name.endsWith('.rs')) continue;
		const file = relative(backend, join(entry.parentPath, entry.name));
		if (file.includes('/target/')) continue;
		files.push({ file, tests: /^crates\/[^/]+\/tests\//.test(file) });
	}
	return files.sort((a, b) => a.file.localeCompare(b.file));
}

async function lineCount(path) {
	const text = await readFile(path, 'utf8');
	return text.split('\n').length - (text.endsWith('\n') ? 1 : 0);
}

/** `#[expect(clippy::<limit lint>, reason = "…")]` (or allow). */
async function clippyExemptions(backend, sources) {
	const pattern = /#\[(?:expect|allow)\(\s*clippy::(too_many_lines|excessive_nesting|too_many_arguments)[\s\S]*?reason\s*=\s*"((?:[^"\\]|\\.)*)"[\s\S]*?\)\]/g;
	const found = [];
	for (const { file } of sources) {
		const text = await readFile(join(backend, file), 'utf8');
		for (const match of text.matchAll(pattern)) {
			const line = text.slice(0, match.index).split('\n').length;
			found.push({ place: `${file}:${line}`, rule: match[1], reason: match[2] });
		}
	}
	return found;
}
