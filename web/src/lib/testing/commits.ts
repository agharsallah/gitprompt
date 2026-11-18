/**
 * Test data builders for a project's commits (the Commits tab): a commit trace of each kind, a
 * group and the endpoint's answer, overridden per test with only the fields the test is about.
 */
import type { CommitGroup, CommitTrace, ProjectCommits, TraceHow } from '#lib/api/types.ts';

let next = 1;

/** A commit that followed prompt `prompt` (`how`: traced or likely), or one made by hand. */
export function trace(how: TraceHow = 'traced', overrides: Partial<CommitTrace> = {}): CommitTrace {
	const n = next++;
	const byHand = how === 'by_hand';
	return {
		commit: {
			hash: `c0ffee${n}`,
			subject: `Change number ${n}`,
			author: 't',
			time: '2026-11-05T10:00:00+01:00',
			files: 2,
			insertions: 12,
			deletions: 3
		},
		prompt: byHand
			? null
			: {
					id: 100 + n,
					received_at: '2026-11-05T09:58:00+01:00',
					branch: 'main',
					text: `Wire the uploader ${n}\nand test it`,
					clipped: false,
					outcome: 'done'
				},
		session: byHand ? null : 'session-a',
		how,
		...overrides
	};
}

export const group = (key: string | null, commits: CommitTrace[]): CommitGroup => ({
	key,
	count: commits.length,
	commits
});

export function projectCommits(overrides: Partial<ProjectCommits> = {}): ProjectCommits {
	const groups = overrides.groups ?? [group('2', [trace('traced'), trace('likely')])];
	const all = groups.flatMap((g) => g.commits);
	const of = (how: TraceHow) => all.filter((t) => t.how === how).length;
	return {
		id: 'L3dvcmsvYXBw',
		repo: '/work/app',
		name: 'app',
		git: true,
		group: 'phase',
		counts: {
			total: all.length,
			traced: of('traced'),
			likely: of('likely'),
			by_hand: of('by_hand')
		},
		matched: all.length,
		groups,
		...overrides
	};
}
