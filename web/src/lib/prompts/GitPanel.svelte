<script lang="ts" module>
	/** "3 files changed, 120 insertions(+), 4 deletions(-)" -> numbers. */
	export function parseDiffstat(stat: string | undefined) {
		if (!stat) return null;
		const num = (re: RegExp) => Number(stat.match(re)?.[1] ?? 0);
		return {
			files: num(/(\d+) files? changed/),
			insertions: num(/(\d+) insertions?\(\+\)/),
			deletions: num(/(\d+) deletions?\(-\)/)
		};
	}
</script>

<script lang="ts">
	import {
		ArrowDown,
		ArrowUp,
		CircleDot,
		FilePlus2,
		GitBranch,
		GitFork,
		Pencil,
		TriangleAlert
	} from '@lucide/svelte';
	import type { PromptDetail } from '#lib/api/types.ts';
	import { integer, plural } from '#lib/shared/format.ts';
	import Badge from '#lib/ui/Badge.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import RelativeTime from '#lib/ui/RelativeTime.svelte';

	/** The repository when the prompt was sent, and what was committed before the next prompt. */
	let { prompt }: { prompt: PromptDetail } = $props();

	const c = $derived(prompt.context);
	const commits = $derived(prompt.commits.commits ?? []);
	const stat = $derived(parseDiffstat(prompt.commits.diffstat));
	const tree = $derived([
		{ label: 'Staged', value: c.git_staged, icon: CircleDot },
		{ label: 'Modified', value: c.git_modified, icon: Pencil },
		{ label: 'Untracked', value: c.git_untracked, icon: FilePlus2 },
		{ label: 'Conflicts', value: c.git_conflicts, icon: TriangleAlert }
	]);
	// Five blocks split like a code review's diffstat: additions, then deletions.
	const blocks = $derived.by(() => {
		if (!stat) return [];
		const total = stat.insertions + stat.deletions;
		const add = total ? Math.round((stat.insertions / total) * 5) : 0;
		return Array.from({ length: 5 }, (_, i) => (i < add ? 'add' : total ? 'del' : 'none'));
	});
</script>

{#if !c.git_root}
	<EmptyState compact title="Not sent from a repository">
		This prompt's working folder wasn't inside a git repository, so there is no branch or commit to
		show.
	</EmptyState>
{:else}
	<div class="git">
		<section class="block">
			<h3>When you sent it</h3>
			<div class="where">
				<span class="branch"><GitBranch size={16} aria-hidden="true" />{c.git_branch ?? 'detached HEAD'}</span>
				{#if c.git_upstream}
					<span class="upstream" title="Compared with {c.git_upstream}">
						<span class="ab"><ArrowUp size={13} aria-hidden="true" />{c.git_ahead ?? 0}<span class="sr-only"> ahead</span></span>
						<span class="ab"><ArrowDown size={13} aria-hidden="true" />{c.git_behind ?? 0}<span class="sr-only"> behind</span></span>
						<span class="mono muted">{c.git_upstream}</span>
					</span>
				{/if}
				{#if c.git_worktree}<Badge icon={GitFork} tone="info">Worktree</Badge>{/if}
				{#if c.git_detached}<Badge tone="warning">Detached</Badge>{/if}
			</div>

			{#if c.git_head}
				<div class="head">
					<code class="hash">{c.git_head.slice(0, 8)}</code>
					<span class="subject">{c.git_head_subject ?? ''}</span>
					<span class="by muted">
						{c.git_head_author ?? ''}
						{#if c.git_head_time}<RelativeTime iso={c.git_head_time} />{/if}
					</span>
				</div>
			{/if}

			<ul class="tree" aria-label="Working tree">
				{#each tree as t (t.label)}
					<li class={[!t.value && 'zero', t.label === 'Conflicts' && t.value && 'conflict']}>
						<t.icon size={14} aria-hidden="true" />
						<span class="n num">{integer(t.value ?? 0)}</span>
						<span>{t.label}</span>
					</li>
				{/each}
			</ul>

			{#if c.git_repo}
				<p class="remote muted">
					{c.git_host ?? 'origin'} <span class="mono">{c.git_repo}</span>
				</p>
			{/if}
		</section>

		<section class="block">
			<h3>Committed afterwards</h3>
			{#if commits.length}
				{#if stat}
					<p class="stat">
						<span>{plural(stat.files, 'file')} changed</span>
						<span class="ins num">+{integer(stat.insertions)}</span>
						<span class="del num">−{integer(stat.deletions)}</span>
						<span class="blocks" aria-hidden="true">
							{#each blocks as b, i (i)}<span class={b}></span>{/each}
						</span>
					</p>
				{/if}
				<ol class="commits">
					{#each commits as commit (commit.hash)}
						<li>
							<span class="node" aria-hidden="true"></span>
							<code class="hash">{commit.hash}</code>
							<span class="subject">{commit.subject}</span>
							<span class="by muted">{commit.author}, <RelativeTime iso={commit.time} /></span>
						</li>
					{/each}
				</ol>
				<p class="muted note">Commits from this prompt until the next prompt of the session.</p>
			{:else}
				<p class="muted">No commits between this prompt and the next one.</p>
			{/if}
		</section>
	</div>
{/if}

<style>
	.git {
		display: grid;
		gap: var(--space-6);
	}

	.block {
		display: grid;
		gap: var(--space-3);
	}

	h3 {
		font-size: var(--text-md);
	}

	.where {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2) var(--space-4);
	}

	.branch {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1_5);
		font-size: var(--text-lg);
		font-weight: var(--weight-semibold);
	}

	.branch :global(svg) {
		color: var(--color-accent);
	}

	.upstream {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		font-size: var(--text-sm);
	}

	.ab {
		display: inline-flex;
		align-items: center;
		gap: 2px;
		font-variant-numeric: tabular-nums;
	}

	.upstream .mono {
		font-size: var(--text-xs);
	}

	.head,
	.commits li {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr);
		gap: 0 var(--space-3);
		align-items: baseline;
	}

	.hash {
		color: var(--color-accent-text);
		font-size: var(--text-xs);
	}

	.subject {
		font-weight: var(--weight-medium);
		overflow-wrap: anywhere;
	}

	.by {
		grid-column: 2;
		font-size: var(--text-xs);
	}

	.tree {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}

	.tree li {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1_5);
		padding: var(--space-1) var(--space-3);
		border-radius: var(--radius-md);
		background: var(--color-surface-sunken);
		font-size: var(--text-sm);
	}

	.tree li :global(svg) {
		color: var(--color-text-muted);
	}

	.tree .n {
		font-weight: var(--weight-semibold);
	}

	.tree .zero {
		color: var(--color-text-muted);
	}

	.tree .conflict {
		background: var(--color-danger-soft);
		color: var(--color-danger-text);
	}

	.remote {
		font-size: var(--text-xs);
	}

	.stat {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
		font-size: var(--text-sm);
	}

	.ins {
		color: var(--color-diff-add-text);
		font-weight: var(--weight-semibold);
	}

	.del {
		color: var(--color-diff-del-text);
		font-weight: var(--weight-semibold);
	}

	.blocks {
		display: inline-flex;
		gap: 2px;
	}

	.blocks span {
		width: 8px;
		height: 8px;
		border-radius: 2px;
		background: var(--color-surface-sunken);
	}

	.blocks .add {
		background: var(--color-success-solid);
	}

	.blocks .del {
		background: var(--color-danger-solid);
	}

	/* Commits on a rail, newest on top, like a git log graph. */
	.commits {
		position: relative;
		display: grid;
		gap: var(--space-3);
		padding-left: var(--space-5);
	}

	.commits::before {
		content: '';
		position: absolute;
		top: 0.5rem;
		bottom: 0.5rem;
		left: 5px;
		width: 2px;
		background: var(--color-border-strong);
	}

	.commits li {
		position: relative;
	}

	.node {
		position: absolute;
		top: 0.3rem;
		left: calc(-1 * var(--space-5));
		width: 12px;
		height: 12px;
		border: 2px solid var(--color-surface);
		border-radius: 50%;
		background: var(--color-accent);
	}

	.note {
		font-size: var(--text-xs);
	}
</style>
