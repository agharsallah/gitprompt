<script lang="ts">
	import type { LucideIcon } from '@lucide/svelte';
	import type { Snippet } from 'svelte';
	import Sparkline from '#lib/ui/charts/Sparkline.svelte';

	/**
	 * One number and what it means. Label in sentence case, value in proportional figures, an
	 * optional hint underneath and an optional trend.
	 */
	let {
		label,
		value,
		hint,
		icon: Icon,
		trend,
		href,
		children
	}: {
		label: string;
		value: string;
		hint?: string;
		icon?: LucideIcon;
		trend?: number[];
		/** Makes the tile a link to the list behind the number. */
		href?: string;
		/** Extra visual under the value (a meter, a segmented bar). */
		children?: Snippet;
	} = $props();
</script>

<svelte:element this={href ? 'a' : 'div'} class={['tile', href && 'link']} {href}>
	<span class="label">
		{#if Icon}<Icon size={14} aria-hidden="true" />{/if}
		{label}
	</span>
	<span class="value-row">
		<span class="value">{value}</span>
		{#if trend && trend.length > 1}<Sparkline values={trend} width={72} height={24} />{/if}
	</span>
	{#if children}<span class="extra">{@render children()}</span>{/if}
	{#if hint}<span class="hint">{hint}</span>{/if}
</svelte:element>

<style>
	.tile {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		min-width: 0;
		padding: var(--space-4);
		border-radius: var(--radius-lg);
		background: var(--color-surface);
		box-shadow: inset 0 0 0 1px var(--color-border);
		color: inherit;
		text-decoration: none;
	}

	.link:hover {
		box-shadow: inset 0 0 0 1px var(--color-border-control);
	}

	.label {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1_5);
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}

	.label :global(svg) {
		color: var(--color-text-muted);
	}

	.value-row {
		display: flex;
		align-items: flex-end;
		justify-content: space-between;
		gap: var(--space-2);
	}

	.value {
		font-size: var(--text-2xl);
		font-weight: var(--weight-semibold);
		font-stretch: var(--stretch-heading);
		letter-spacing: var(--tracking-tight);
		line-height: var(--leading-tight);
		white-space: nowrap;
	}

	.extra {
		margin-top: var(--space-1);
	}

	.hint {
		color: var(--color-text-muted);
		font-size: var(--text-xs);
	}
</style>
