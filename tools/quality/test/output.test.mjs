// Live output: whole lines from chunks, and what cargo and vitest are doing from their lines.
import assert from 'node:assert/strict';
import { test } from 'node:test';

import { cargoActivity, lineSplitter } from '../lib/output.mjs';
import { vitestActivity } from '../reports/web/10-tests.mjs';

test('lines come whole across chunks, without colour codes, \\r ending a line, blank ones dropped', () => {
	const lines = [];
	const split = lineSplitter((line) => lines.push(line));
	split.push('first li');
	split.push('ne\n\x1b[32mgreen\x1b[0m\n\n10%\r20%\rlast');
	split.end();
	assert.deepEqual(lines, ['first line', 'green', '10%', '20%', 'last']);
});

test('cargo: crates compiled, the test binary running, and tests passed and failed so far', () => {
	const activity = cargoActivity();
	const say = (line) => activity(line);
	assert.equal(say('   Compiling serde v1.0.229'), 'compiling serde (1 crates so far)');
	assert.equal(say('    Checking gitprompt-core v0.1.0 (/repo/backend/crates/gitprompt-core)'), 'compiling gitprompt-core (2 crates so far)');
	assert.equal(say('    Blocking waiting for file lock on build directory'), 'waiting for another cargo command to release backend/target');
	assert.equal(say('    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 02s'), 'built (2 crates compiled)');
	assert.equal(say('     Running unittests src/lib.rs (target/llvm-cov-target/debug/deps/gitprompt_core-0a1b2c3d)'), 'testing gitprompt_core src/lib.rs · 0 passed');
	assert.equal(say('test model::tests::parses ... ok'), 'testing gitprompt_core src/lib.rs · 1 passed');
	assert.equal(say('test model::tests::breaks ... FAILED'), 'testing gitprompt_core src/lib.rs · 1 passed, 1 failed');
	assert.equal(say('     Running tests/api_snapshots.rs (target/debug/deps/api_snapshots-99ff00aa)'), 'testing api_snapshots tests/api_snapshots.rs · 1 passed, 1 failed');
	assert.equal(say('   Doc-tests gitprompt_store'), 'testing gitprompt_store doc tests · 1 passed, 1 failed');
	assert.equal(say('running 12 tests'), undefined);
});

test('vitest: test files done, and tests passed and failed so far', () => {
	const activity = vitestActivity();
	assert.equal(activity(' ✓ |unit| src/lib/api/client.test.ts (6 tests) 24ms'), '1 test files done · 6 tests passed · src/lib/api/client.test.ts');
	assert.equal(activity(' ❯ |unit| src/lib/x.test.ts (5 tests | 2 failed) 9ms'), '2 test files done · 9 tests passed, 2 failed · src/lib/x.test.ts');
	assert.equal(activity('   ✓ theme (3)'), undefined);
	assert.equal(activity(' Test Files  44 passed (44)'), undefined);
});
