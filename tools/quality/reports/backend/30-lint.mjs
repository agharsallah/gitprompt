// Clippy (the workspace's pedantic set; CI turns every warning into an error) and rustfmt: every
// finding, by lint, by crate and by place.
import { groupBy, kpis, note, plural, table, toneZero } from '../../lib/spec.mjs';

/** Findings listed one by one (the rest are counted). */
const LISTED = 300;

export default {
	title: 'Lint',
	description: 'Clippy findings (pedantic; warnings fail CI) and files rustfmt would change.',
	async run(ctx) {
		const clippy = await ctx.exec('cargo', ['clippy', '--workspace', '--all-targets', '--message-format=json', '-q'], {
			cwd: ctx.backend,
			label: 'cargo clippy --workspace --all-targets',
			allowFailure: true
		});
		const findings = parseClippy(clippy.stdout);
		const fmt = await ctx.exec('cargo', ['fmt', '--all', '--check'], { cwd: ctx.backend, label: 'cargo fmt --check', allowFailure: true });
		const unformatted = [...new Set([...fmt.stdout.matchAll(/^Diff in (.+?)(?: at line \d+)?:$/gm)].map((m) => m[1].replace(`${ctx.repo}/`, '')))];
		return page(findings, unformatted);
	}
};

/** Compiler messages with a lint or an error, de-duplicated (lib and test builds repeat them). */
export function parseClippy(stdout) {
	const seen = new Set();
	const findings = [];
	for (const line of stdout.split('\n')) {
		if (!line.startsWith('{')) continue;
		const entry = JSON.parse(line);
		const message = entry.message;
		if (entry.reason !== 'compiler-message' || !['warning', 'error'].includes(message.level)) continue;
		const span = message.spans.find((s) => s.is_primary);
		if (!span) continue; // "N warnings emitted"
		const lint = message.code?.code || (message.level === 'error' ? 'error' : 'warning');
		const key = `${lint}|${span.file_name}|${span.line_start}|${message.message}`;
		if (seen.has(key)) continue;
		seen.add(key);
		findings.push({ level: message.level, lint, message: message.message, file: span.file_name, line: span.line_start, crate: crateOf(entry.package_id) });
	}
	return findings;
}

/** `path+file:///…/crates/gitprompt-core#0.2.0` or `…#gitprompt-core@0.2.0` → gitprompt-core. */
function crateOf(packageId) {
	const [path, fragment = ''] = packageId.split('#');
	return fragment.includes('@') ? fragment.split('@')[0] : path.split('/').pop();
}

function lintLink(lint) {
	return lint.startsWith('clippy::') ? `https://rust-lang.github.io/rust-clippy/master/index.html#${lint.slice(8)}` : undefined;
}

function page(findings, unformatted) {
	const errors = findings.filter((f) => f.level === 'error').length;
	const warnings = findings.length - errors;
	const total = findings.length + unformatted.length;
	const group = (key) => groupBy(findings, (f) => f[key]);
	const sections = [
		kpis([
			{ label: 'Clippy warnings', value: warnings, tone: toneZero(warnings) },
			{ label: 'Errors', value: errors, tone: toneZero(errors) },
			{ label: 'Files rustfmt would change', value: unformatted.length, tone: toneZero(unformatted.length) },
			{ label: 'Lints hit', value: new Set(findings.map((f) => f.lint)).size }
		])
	];
	if (total === 0) {
		sections.push(note('Clean: nothing for `just lint` to fail on (CI runs clippy with -D warnings).', 'good'));
	} else {
		sections.push(note('CI fails on every one of these: `just lint` runs clippy with -D warnings. Fix the cause; an #[allow] needs a reason.', 'bad'));
	}
	if (findings.length) {
		sections.push(
			table(
				'By lint',
				[
					{ key: 'lint', label: 'Lint', mono: true },
					{ key: 'count', label: 'Findings', align: 'right' },
					{ key: 'example', label: 'Example' }
				],
				group('lint').map(([lint, list]) => ({ lint: { text: lint, href: lintLink(lint) }, count: list.length, example: list[0].message }))
			),
			table(
				'By crate',
				[
					{ key: 'crate', label: 'Crate', mono: true },
					{ key: 'count', label: 'Findings', align: 'right' }
				],
				group('crate').map(([crate, list]) => ({ crate, count: list.length }))
			),
			table(
				'Findings',
				[
					{ key: 'place', label: 'Where', mono: true },
					{ key: 'lint', label: 'Lint', mono: true },
					{ key: 'message', label: 'Message' }
				],
				findings.slice(0, LISTED).map((f) => ({ place: `${f.file}:${f.line}`, lint: { text: f.lint, tone: f.level === 'error' ? 'bad' : 'warn' }, message: f.message })),
				findings.length > LISTED ? `First ${LISTED} of ${findings.length}.` : undefined
			)
		);
	}
	if (unformatted.length) {
		sections.push(table('Files rustfmt would change (run `just fmt`)', [{ key: 'file', label: 'File', mono: true }], unformatted.map((file) => ({ file }))));
	}
	return {
		headline: { value: total, label: total === 1 ? 'finding' : 'findings', tone: toneZero(total) },
		summary: `${plural(warnings, 'clippy warning')}, ${plural(errors, 'error')}, ${plural(unformatted.length, 'file')} to format`,
		sections
	};
}
