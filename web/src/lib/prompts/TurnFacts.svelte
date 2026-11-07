<script lang="ts">
	import { Bot, Coins, FilePen, GitCommitHorizontal, Timer, Wrench } from '@lucide/svelte';
	import type { Prompt } from '#lib/api/types.ts';
	import { factsView } from '#lib/prompts/facts.ts';
	import { duration } from '#lib/shared/format.ts';

	/**
	 * What a turn amounted to, in a single quiet line: how long the tool worked, how many tool calls,
	 * files changed, tokens and cost when the tool reported them, and the model. Each fact carries an
	 * icon and a tooltip that says what it is. Read from the daemon's `facts` (prompts/facts.ts).
	 */
	let { prompt, models = true }: { prompt: Prompt; models?: boolean } = $props();

	const f = $derived(prompt.facts);
	const view = $derived(factsView(f));
</script>

<span class="facts">
	{#if f.response_s != null}
		<span class="fact" title="Time until the tool finished">
			<Timer size={13} aria-hidden="true" /><span class="sr-only">Worked for</span>{duration(
				f.response_s
			)}
		</span>
	{/if}
	{#if f.tool_calls}
		<span class="fact" title="Tool calls{f.tool_failures ? `, ${f.tool_failures} failed` : ''}">
			<Wrench size={13} aria-hidden="true" />{f.tool_calls}<span class="sr-only"> tool calls</span>
			{#if f.tool_failures}<span class="failures"
					>{f.tool_failures}<span class="sr-only"> failed</span></span
				>{/if}
		</span>
	{/if}
	{#if f.files_written}
		<span class="fact" title="Files edited or written">
			<FilePen size={13} aria-hidden="true" />{f.files_written}<span class="sr-only">
				files changed</span
			>
		</span>
	{/if}
	{#if f.git_commits}
		<span class="fact" title="Ran git commit">
			<GitCommitHorizontal size={13} aria-hidden="true" /><span class="sr-only">Committed</span>
		</span>
	{/if}
	{#if view.tokens}
		<span class="fact" title={view.tokens.hint}>
			<Coins size={13} aria-hidden="true" />{view.tokens.total}<span class="sr-only"> tokens</span>
		</span>
	{/if}
	{#if view.cost}
		<span class="fact" title="Cost of this turn">{view.cost}</span>
	{/if}
	{#if models && view.model}
		<span class="fact model" title={view.model.hint}
			><Bot size={13} aria-hidden="true" /><span class="sr-only">Model </span>{view.model
				.name}</span
		>
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

	.model,
	.model :global(svg) {
		color: var(--color-agent-text);
	}
</style>
