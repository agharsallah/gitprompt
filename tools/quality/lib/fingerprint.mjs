// What a report's result depends on, as one hash: every file under its inputs, path and content, as
// it is in the working tree. Git does the work: the index gives each tracked file's content id, and
// only the files edited, deleted or not tracked yet are hashed again (as git would store them, so a
// commit or `git add` never changes the hash, only content does). Ignored files (target/,
// node_modules/, build output) never count. A report whose inputs hash the same as at its last
// successful run is up to date and is not rebuilt.
import { execFile } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { promisify } from 'node:util';

const run = promisify(execFile);

/** git's output; `input` goes to its stdin. Paths come NUL-separated (-z), never quoted. */
async function git(repo, args, input) {
	const pending = run('git', args, { cwd: repo, maxBuffer: 64 * 1024 * 1024 });
	if (input !== undefined) pending.child.stdin.end(input);
	return (await pending).stdout;
}

const entries = (text) => text.split('\0').filter(Boolean);

/** path → content id of every file under `paths`, as in the working tree. */
async function contentIds(repo, paths) {
	const scope = ['--', ...[...paths].sort()];
	const [indexed, changed] = await Promise.all([
		// `<mode> <id> <stage>\t<path>` of every tracked file, as staged.
		git(repo, ['ls-files', '--stage', '-z', ...scope]),
		// Tracked files edited or deleted since, and files not tracked yet (unless ignored).
		git(repo, ['ls-files', '--modified', '--others', '--exclude-standard', '-z', ...scope])
	]);
	const ids = new Map(entries(indexed).map((entry) => [entry.slice(entry.indexOf('\t') + 1), entry.split(' ')[1]]));
	const edited = [...new Set(entries(changed))];
	for (const file of edited) ids.delete(file);
	const present = edited.filter((file) => existsSync(join(repo, file)));
	if (present.length > 0) {
		const hashed = (await git(repo, ['hash-object', '--stdin-paths'], `${present.join('\n')}\n`)).split('\n');
		present.forEach((file, index) => ids.set(file, hashed[index]));
	}
	return ids;
}

/** A short hash of the files under `paths` (repository-relative files or folders) in `repo`. */
export async function fingerprint(repo, paths) {
	const ids = await contentIds(repo, paths);
	const hash = createHash('sha256');
	for (const file of [...ids.keys()].sort()) hash.update(`${ids.get(file)} ${file}\0`);
	return hash.digest('hex').slice(0, 16);
}

/**
 * The reports that can keep their last result: it succeeded, from the same inputs (hashes: { id: hash }),
 * and its page is still on disk (hasPage(report)). A Set of ids.
 */
export async function upToDate(reports, records, hashes, hasPage) {
	const kept = await Promise.all(
		reports.map(async (report) => {
			const last = records[report.id];
			return last?.state === 'done' && last.inputs === hashes[report.id] && (await hasPage(report));
		})
	);
	return new Set(reports.filter((_, index) => kept[index]).map((report) => report.id));
}

/**
 * fingerprint() for many reports at once, each input list hashed once: { [report id]: hash }.
 * inputsOf(report) → its paths.
 */
export async function fingerprints(repo, reports, inputsOf) {
	const byKey = new Map();
	const hashOf = (paths) => {
		const key = [...paths].sort().join('\n');
		if (!byKey.has(key)) byKey.set(key, fingerprint(repo, paths));
		return byKey.get(key);
	};
	const entries = await Promise.all(reports.map(async (report) => [report.id, await hashOf(inputsOf(report))]));
	return Object.fromEntries(entries);
}
