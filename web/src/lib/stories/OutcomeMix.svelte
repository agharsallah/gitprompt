<script lang="ts">
	import type { TurnOutcome } from '#lib/api/types.ts';
	import { TURN_OUTCOMES } from '#lib/insights/outcome.ts';
	import { plural } from '#lib/shared/format.ts';
	import SegmentedBar from '#lib/ui/charts/SegmentedBar.svelte';
	import { OUTCOME_COLORS, OUTCOME_ORDER } from '#lib/stories/story.ts';

	/**
	 * How a set of turns ended, as one bar (outcome colours of the rail) with the counts in words
	 * below, so the mix never relies on colour.
	 */
	let {
		outcomes,
		label,
		words = true
	}: { outcomes: Partial<Record<TurnOutcome, number>>; label: string; words?: boolean } = $props();

	const shown = $derived(OUTCOME_ORDER.filter((o) => outcomes[o]));
</script>

<div class="outcomes">
	<SegmentedBar
		{label}
		parts={shown.map((o) => ({
			key: o,
			label: TURN_OUTCOMES[o].label,
			value: outcomes[o] ?? 0,
			color: OUTCOME_COLORS[o]
		}))}
		format={(v) => plural(v, 'prompt')}
		decorative={words}
	/>
	{#if words}
		<p class="words">
			{#each shown as o (o)}<span
					><span class="swatch" style:background={OUTCOME_COLORS[o]} aria-hidden="true"
					></span>{outcomes[o]}
					{TURN_OUTCOMES[o].label.toLowerCase()}</span
				>{/each}
		</p>
	{/if}
</div>

<style>
	.outcomes {
		display: grid;
		gap: var(--space-1_5);
	}

	.words {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1) var(--space-3);
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
		font-variant-numeric: tabular-nums;
	}

	.words > span {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
	}

	.swatch {
		width: 8px;
		height: 8px;
		border-radius: 2px;
	}
</style>
