// When a report counts as up to date: its inputs hash the same as at its last success, as git sees them.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, rmSync, unlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { after, before, test } from 'node:test';

import { fingerprint, upToDate } from '../lib/fingerprint.mjs';

let repo;
const git = (...args) => execFileSync('git', args, { cwd: repo, stdio: 'pipe' });
const write = (file, text) => writeFileSync(join(repo, file), text);
const hash = () => fingerprint(repo, ['web']);

before(() => {
	repo = mkdtempSync(join(tmpdir(), 'quality-fingerprint-'));
	git('init', '-q');
	execFileSync('mkdir', ['-p', join(repo, 'web/src'), join(repo, 'backend')]);
	write('.gitignore', 'web/build/\n');
	write('web/src/a.ts', 'export const a = 1;\n');
	write('backend/lib.rs', 'fn main() {}\n');
	git('add', '.');
	git('-c', 'user.name=t', '-c', 'user.email=t@t', 'commit', '-qm', 'init');
});

after(() => rmSync(repo, { recursive: true, force: true }));

test('the same files hash the same; an edit, a new file or a deletion changes it, undoing it restores it', async () => {
	const clean = await hash();
	assert.equal(await hash(), clean);

	write('web/src/a.ts', 'export const a = 2;\n');
	const edited = await hash();
	assert.notEqual(edited, clean);
	write('web/src/a.ts', 'export const a = 1;\n');
	assert.equal(await hash(), clean);

	write('web/src/new.ts', 'export {};\n');
	assert.notEqual(await hash(), clean);
	unlinkSync(join(repo, 'web/src/new.ts'));

	unlinkSync(join(repo, 'web/src/a.ts'));
	assert.notEqual(await hash(), clean);
	git('checkout', '-q', '--', 'web/src/a.ts');
	assert.equal(await hash(), clean);
});

test('staging or committing an edit keeps the hash: only content counts', async () => {
	write('web/src/a.ts', 'export const a = 3;\n');
	const edited = await hash();
	git('add', 'web/src/a.ts');
	assert.equal(await hash(), edited);
	git('-c', 'user.name=t', '-c', 'user.email=t@t', 'commit', '-qm', 'edit');
	assert.equal(await hash(), edited);
	write('web/src/b.ts', 'export {};\n');
	const added = await hash();
	git('add', 'web/src/b.ts');
	git('-c', 'user.name=t', '-c', 'user.email=t@t', 'commit', '-qm', 'add');
	assert.equal(await hash(), added);
	git('rm', '-q', 'web/src/b.ts');
	write('web/src/a.ts', 'export const a = 1;\n');
	git('-c', 'user.name=t', '-c', 'user.email=t@t', 'commit', '-qam', 'back');
});

test('ignored files and files outside the inputs do not count', async () => {
	const clean = await hash();
	execFileSync('mkdir', ['-p', join(repo, 'web/build')]);
	write('web/build/app.js', 'built');
	write('backend/lib.rs', 'fn main() { changed(); }\n');
	assert.equal(await hash(), clean);
	git('checkout', '-q', '--', 'backend/lib.rs');
});

test('a report is up to date only after a success from the same inputs, with its page on disk', async () => {
	const reports = [{ id: 'same' }, { id: 'changed' }, { id: 'failed' }, { id: 'no-page' }, { id: 'never' }];
	const records = {
		same: { state: 'done', inputs: 'h1' },
		changed: { state: 'done', inputs: 'old' },
		failed: { state: 'failed', inputs: 'h1' },
		'no-page': { state: 'done', inputs: 'h1' }
	};
	const hashes = { same: 'h1', changed: 'h1', failed: 'h1', 'no-page': 'h1', never: 'h1' };
	const kept = await upToDate(reports, records, hashes, async (report) => report.id !== 'no-page');
	assert.deepEqual([...kept], ['same']);
});
