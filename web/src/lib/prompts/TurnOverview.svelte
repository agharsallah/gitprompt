<script lang="ts">
	import type { PromptDetail } from '#lib/api/types.ts';
	import { turnFigures } from '#lib/prompts/detail.ts';
	import type { FactsView } from '#lib/prompts/facts.ts';
	import TurnTimeline from '#lib/prompts/TurnTimeline.svelte';

	/**
	 * How the turn went: its figures (time, tool calls, files, tokens, cost), why tokens are missing
	 * when they are, and the timeline of what the tool did.
	 */
	let {
		prompt,
		view,
		noTokens
	}: { prompt: PromptDetail; view: FactsView; noTokens: string | null } = $props();

	const s = $derived(prompt.summary);
	const figures = $derived(turnFigures(s, view, prompt.facts));
</script>

<section class="turn" aria-label="How the turn went">
	<dl class="figures">
		{#each figures as fig (fig.label)}
			<div title={fig.title}>
				<dt>{fig.label}</dt>
				<dd>
					{fig.value}
					{#if fig.hint}<span class="hint">{fig.hint}</span>{/if}
					{#if fig.note}<span class="hint quiet">{fig.note}</span>{/if}
				</dd>
			</div>
		{/each}
	</dl>
	{#if noTokens}<p class="no-tokens muted">{noTokens}</p>{/if}
	<TurnTimeline
		events={prompt.events}
		start={prompt.received_at}
		responseS={s.response_s}
		kinds={s.tool_kind_of}
	/>
</section>

<style>
	.turn {
		display: grid;
		gap: var(--space-4);
		padding: var(--space-4);
		border-radius: var(--radius-lg);
		background: var(--color-surface-sunken);
	}

	.figures {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-3) var(--space-6);
		margin: 0;
	}

	.figures div {
		display: grid;
	}

	dt {
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
	}

	dd {
		display: flex;
		align-items: baseline;
		gap: var(--space-1_5);
		margin: 0;
		font-size: var(--text-xl);
		font-weight: var(--weight-semibold);
		font-stretch: var(--stretch-heading);
	}

	.hint {
		color: var(--color-danger-text);
		font-size: var(--text-xs);
		font-weight: var(--weight-medium);
	}

	.hint.quiet {
		color: var(--color-text-muted);
	}

	.no-tokens {
		margin: calc(-1 * var(--space-2)) 0 0;
		font-size: var(--text-sm);
	}
</style>
