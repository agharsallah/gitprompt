// The time-dependency check (tools/timing): what each rule matches where, which matches block and
// which only report, the ledger of accepted lines, and how `just flaky` reads a run.
import assert from 'node:assert/strict';
import { test } from 'node:test';

import { failedTests, options } from '../../timing/flaky.mjs';
import { compare, ledgerOf, shrink } from '../../timing/ledger.mjs';
import { codeOf, isTestFile, scanText } from '../../timing/scan.mjs';

const rules = (file, text) => scanText(file, text).map((f) => `${f.line}:${f.rule}:${f.policy}`);

test('in tests a sleep, a raw timeout and a timing assertion block; polling with eventually reports', () => {
	const rust = [
		'fn wait() {',
		'    std::thread::sleep(POLL);',
		'    rx.recv_timeout(Duration::from_secs(5));',
		'    assert!(started.elapsed() < Duration::from_secs(5));',
		'    eventually(DEADLINE, "x", || None::<()>);',
		'}'
	].join('\n');
	assert.deepEqual(rules('backend/crates/x/tests/a.rs', rust), ['2:sleep:block', '3:deadline:block', '4:elapsed-assert:block', '5:poll:report']);
});

test('in code a timer or a timeout reports; a Rust file is test code from its #[cfg(test)] on', () => {
	const text = ['fn run() { tokio::time::sleep(every); }', '#[cfg(test)]', 'mod tests {', '    fn t() { thread::sleep(MS); }', '}'].join('\n');
	assert.deepEqual(rules('backend/crates/x/src/gate.rs', text), ['1:timer:report', '4:sleep:block']);
});

test('a clock read blocks in code, reports in tests and in the clock seams; an injected default never counts', () => {
	const text = [
		'export function relative(iso: string, now = Date.now()): string {',
		'export function shortDate(iso: Date, now = new Date()) {',
		'\tnow = Date.now()',
		'const now = Date.now();',
		'load(filters, Date.now());'
	].join('\n');
	assert.deepEqual(rules('web/src/lib/a.ts', text), ['4:wall-clock:block', '5:wall-clock:block']);
	assert.deepEqual(rules('web/src/lib/a.test.ts', 'const now = Date.now();'), ['1:wall-clock:report']);
	assert.deepEqual(rules('backend/crates/x/src/a.rs', 'let now = Timestamp::now();'), ['1:wall-clock:block']);
	assert.deepEqual(rules('backend/crates/gitprompt-server/src/clock.rs', 'Timestamp::now()'), ['1:wall-clock:report']);
	assert.deepEqual(rules('backend/crates/gitprompt-cli/src/doctor/mod.rs', 'gather(settings, Timestamp::now())'), ['1:wall-clock:report']);
});

test('comments never count, a URL is not a comment, and test files are told by name and folder', () => {
	assert.deepEqual(rules('backend/crates/x/src/a.rs', ['/// never from `Timestamp::now()`', '// thread::sleep(x)', ' * Instant::now()'].join('\n')), []);
	assert.equal(codeOf('let u = "http://x"; // Instant::now()').trimEnd(), 'let u = "http://x";');
	assert.ok(isTestFile('web/src/lib/a.test.ts') && isTestFile('backend/crates/x/tests/common/mod.rs'));
	assert.ok(!isTestFile('web/src/lib/a.ts') && !isTestFile('backend/crates/x/src/testing.rs'));
	assert.deepEqual(rules('web/src/lib/a.test.ts', 'const settle = () => new Promise((r) => setTimeout(r));'), ['1:sleep:block']);
	const hooks = ['beforeAll(async () => {', '}, 120_000);', 'beforeAll(async () => {', '}, 0);', "it('x', () => {}, 5000);"].join('\n');
	assert.deepEqual(rules('web/src/lib/a.test.ts', hooks), ['2:deadline:block', '5:deadline:block'], 'a timeout argument blocks; 0 (no deadline) does not');
});

test('the ledger accepts lines, not counts: a moved line passes, a swapped one fails, a reported one never counts', () => {
	const at = (file, rule, text, line = 1, policy = 'block') => ({ file, rule, text, line, policy, scope: 'test' });
	const ledger = ledgerOf([at('a.rs', 'sleep', 'thread::sleep(POLL);'), at('a.rs', 'sleep', 'thread::sleep(POLL);'), at('b.rs', 'deadline', 'rx.recv_timeout(T);')]);
	assert.deepEqual(ledger, { 'a.rs': { sleep: ['thread::sleep(POLL);', 'thread::sleep(POLL);'] }, 'b.rs': { deadline: ['rx.recv_timeout(T);'] } });

	const moved = [at('a.rs', 'sleep', '  thread::sleep(POLL);', 40), at('a.rs', 'sleep', 'thread::sleep(POLL);', 90), at('b.rs', 'deadline', 'rx.recv_timeout(T);', 7)];
	assert.deepEqual(compare(moved, ledger), { added: [], gone: [] });

	const swapped = [at('a.rs', 'sleep', 'thread::sleep(POLL);'), at('a.rs', 'sleep', 'thread::sleep(LONGER);', 9), at('c.rs', 'stopwatch', 'Instant::now()', 1, 'report')];
	const { added, gone } = compare(swapped, ledger);
	assert.deepEqual(
		added.map((f) => `${f.file}:${f.line}`),
		['a.rs:9']
	);
	assert.deepEqual(gone, [
		{ file: 'a.rs', rule: 'sleep', text: 'thread::sleep(POLL);' },
		{ file: 'b.rs', rule: 'deadline', text: 'rx.recv_timeout(T);' }
	]);
	assert.deepEqual(shrink(ledger, swapped), { 'a.rs': { sleep: ['thread::sleep(POLL);'] } }, 'drops what is gone, never adds the new line');
});

test('just flaky: its options, and the tests a failed run names', () => {
	assert.deepEqual(options([]), { runs: 8, jobs: 4, web: false, cargo: ['--workspace'] });
	assert.deepEqual(options(['--runs', '20', '--web', '--', '-p', 'gitprompt-cli']), { runs: 20, jobs: 4, web: true, cargo: ['-p', 'gitprompt-cli'] });
	assert.throws(() => options(['--jobs', '0']), /--jobs takes a whole number/);
	const cargo = 'failures:\n\n---- serve_answers_healthz stdout ----\nthread panicked\n\n---- hook::spooled stdout ----\n';
	assert.deepEqual(failedTests(cargo), ['serve_answers_healthz', 'hook::spooled']);
	assert.deepEqual(failedTests(' FAIL  src/lib/a.test.ts > relative > says today\n'), ['src/lib/a.test.ts > relative > says today']);
});
