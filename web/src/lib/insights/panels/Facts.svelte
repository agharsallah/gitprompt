<script lang="ts" module>
	import type { LucideIcon } from '@lucide/svelte';

	export type Fact = {
		label: string;
		value: string;
		/** The prompts behind the number. */
		href?: string;
		hint?: string;
		icon?: LucideIcon;
	};
</script>

<script lang="ts">
	/** A few counts, each a link to the prompts it counts: the figures beside a chart. */
	let { facts, label }: { facts: Fact[]; label: string } = $props();
</script>

<ul class="facts" aria-label={label}>
	{#each facts as f (f.label)}
		<li>
			{#if f.icon}<f.icon size={14} aria-hidden="true" />{/if}
			<span class="text">
				{#if f.href}<a href={f.href}>{f.label}</a>{:else}{f.label}{/if}
				{#if f.hint}<span class="hint">{f.hint}</span>{/if}
			</span>
			<span class="value">{f.value}</span>
		</li>
	{/each}
</ul>

<style>
	.facts {
		display: grid;
		gap: var(--space-1);
		font-size: var(--text-sm);
	}

	li {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr) auto;
		align-items: baseline;
		gap: var(--space-2);
		padding: var(--space-1) 0;
		border-bottom: var(--border-hairline) solid var(--color-border);
		color: var(--color-text-secondary);
	}

	li:not(:has(> :global(svg))) {
		grid-template-columns: minmax(0, 1fr) auto;
	}

	li > :global(svg) {
		align-self: center;
		color: var(--color-text-muted);
	}

	li:last-child {
		border-bottom: 0;
	}

	.text {
		min-width: 0;
		overflow-wrap: anywhere;
	}

	a {
		color: var(--color-text);
	}

	.hint {
		display: block;
		color: var(--color-text-muted);
		font-size: var(--text-xs);
	}

	.value {
		color: var(--color-text);
		font-weight: var(--weight-semibold);
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
	}
</style>
