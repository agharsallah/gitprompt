<script lang="ts">
	import { FilePen, GitCommitHorizontal, Timer, Wrench } from '@lucide/svelte';
	import type { StoryCommit } from '#lib/api/types.ts';
	import { duration, integer, money, plural } from '#lib/shared/format.ts';
	import { diffstatText } from '#lib/stories/story.ts';

	/**
	 * What one turn of a story amounted to, in a quiet line: how long, tool calls, files, commits,
	 * cost. Each fact has an icon and words for screen readers (the same facts as TurnFacts, from a
	 * story's data instead of a prompt's).
	 */
	let {
		turn
	}: {
		turn: {
			response_s: number | null;
			tool_calls: number;
			tool_failures: number;
			files_written: number;
			cost_usd: number;
			commits: StoryCommit[];
			git_commits: number;
		};
	} = $props();

	const changes = $derived(
		diffstatText({
			files: turn.commits.reduce((n, c) => n + c.files, 0),
			insertions: turn.commits.reduce((n, c) => n + c.insertions, 0),
			deletions: turn.commits.reduce((n, c) => n + c.deletions, 0)
		})
	);
</script>

<span class="facts">
	{#if turn.response_s != null}
		<span class="fact" title="Time until the tool finished"
			><Timer size={13} aria-hidden="true" /><span class="sr-only">Worked for </span>{duration(
				turn.response_s
			)}</span
		>
	{/if}
	{#if turn.tool_calls}
		<span class="fact" title="Tool calls"
			><Wrench size={13} aria-hidden="true" />{integer(turn.tool_calls)}<span class="sr-only">
				tool calls</span
			>{#if turn.tool_failures}<span class="failures"
					>{turn.tool_failures}<span class="sr-only"> failed</span></span
				>{/if}</span
		>
	{/if}
	{#if turn.files_written}
		<span class="fact" title="Files edited or written"
			><FilePen size={13} aria-hidden="true" />{integer(turn.files_written)}<span class="sr-only">
				files changed</span
			></span
		>
	{/if}
	{#if turn.commits.length}
		<span class="fact" title="Commits made after this prompt, before the next one"
			><GitCommitHorizontal size={13} aria-hidden="true" />{plural(turn.commits.length, 'commit')}
			{#if changes}<span class="changes">{changes}</span>{/if}</span
		>
	{:else if turn.git_commits}
		<span class="fact" title="The agent ran git commit"
			><GitCommitHorizontal size={13} aria-hidden="true" />Ran git commit</span
		>
	{/if}
	{#if turn.cost_usd}
		<span class="fact" title="Cost of this turn (telemetry)">{money(turn.cost_usd)}</span>
	{/if}
</span>

<style>
	.facts {
		display: inline-flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1) var(--space-3);
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
		font-variant-numeric: tabular-nums;
	}

	.fact {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
		white-space: nowrap;
	}

	.fact :global(svg) {
		color: var(--color-text-muted);
	}

	.failures {
		margin-left: 2px;
		padding: 0 4px;
		border-radius: var(--radius-full);
		background: var(--color-danger-soft);
		color: var(--color-danger-text);
		font-weight: var(--weight-semibold);
	}

	.changes {
		color: var(--color-text-muted);
	}
</style>
