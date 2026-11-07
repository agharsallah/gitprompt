import type { Prompt } from '#lib/api/types.ts';
import { folder } from '#lib/shared/format.ts';

/**
 * The project a prompt belongs to: its repository root when it was sent inside one (the same key
 * the prompt list's `repo` filter uses), else its working directory.
 */
export type ProjectRef = {
	key: string;
	name: string;
	/** git_root: set when the prompt was sent inside a repository (filterable). */
	root?: string;
	/** owner/name from the origin remote, when there is one. */
	repo?: string;
	host?: string;
};

export const NO_FOLDER = '(no folder)';

export function projectOf(p: Prompt): ProjectRef {
	const c = p.context;
	const root = c.git_root;
	const key = root ?? p.fields.cwd ?? NO_FOLDER;
	return {
		key,
		name: c.git_repo ?? folder(root) ?? folder(p.fields.cwd) ?? 'No folder',
		root,
		repo: c.git_repo,
		host: c.git_host
	};
}

/**
 * Merge what several prompts say about one project: a prompt sent before the remote was added
 * knows less than a later one, so the most informative value wins.
 */
export function mergeProject(a: ProjectRef, b: ProjectRef): ProjectRef {
	return {
		key: a.key,
		root: a.root ?? b.root,
		repo: a.repo ?? b.repo,
		host: a.host ?? b.host,
		name: a.repo ?? b.repo ?? a.name
	};
}
