// Where things are: the tool, the repository, and the dashboard it writes (git-ignored).
import { dirname, join, relative, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

export const TOOL = dirname(dirname(fileURLToPath(import.meta.url)));
export const REPO = dirname(dirname(TOOL));
export const OUT = process.env.QUALITY_OUT || join(REPO, 'target', 'quality');

export const PATHS = {
	hub: join(TOOL, 'hub'),
	reports: join(TOOL, 'reports'),
	index: join(OUT, 'index.html'),
	status: join(OUT, 'status.js'),
	state: join(OUT, 'state.json'),
	data: join(OUT, 'data'),
	logs: join(OUT, 'logs'),
	artifacts: join(OUT, 'artifacts')
};

/** `path` relative to the dashboard page, with forward slashes: what a link or frame needs. */
export function fromHub(path) {
	return relative(OUT, path).split(sep).join('/');
}

/** `path` relative to the repository, for messages. */
export function fromRepo(path) {
	return relative(REPO, path).split(sep).join('/');
}
