<script lang="ts">
	import type { Snippet } from 'svelte';

	/**
	 * The hover/focus readout of a chart, placed above a point inside a `position: relative` chart.
	 * Hidden from assistive tech: every value it shows is also in the mark's accessible name or the
	 * chart's table view (tooltips enhance, never gate).
	 */
	let {
		x,
		y,
		width,
		children
	}: {
		/** Anchor point, in px from the chart's top-left. */
		x: number;
		y: number;
		/** Chart width: the tooltip is kept inside it. */
		width: number;
		children: Snippet;
	} = $props();

	let box = $state(0);
	const left = $derived(Math.min(Math.max(x - box / 2, 0), Math.max(width - box, 0)));
</script>

<div class="tooltip" aria-hidden="true" bind:clientWidth={box} style:left="{left}px" style:top="{y}px">
	{@render children()}
</div>

<style>
	.tooltip {
		position: absolute;
		z-index: var(--z-tooltip);
		max-width: 18rem;
		padding: var(--space-2) var(--space-3);
		border-radius: var(--radius-md);
		background: var(--color-surface-raised);
		box-shadow: var(--shadow-overlay);
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
		line-height: var(--leading-snug);
		pointer-events: none;
		transform: translateY(calc(-100% - 8px));
	}

	.tooltip :global(strong) {
		display: block;
		color: var(--color-text);
		font-size: var(--text-sm);
		font-weight: var(--weight-semibold);
	}
</style>
