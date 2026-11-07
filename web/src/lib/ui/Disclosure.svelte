<script lang="ts">
	import { ChevronRight } from '@lucide/svelte';
	import type { Snippet } from 'svelte';

	/** Native <details>: keyboard and find-in-page work for free. */
	let {
		summary,
		meta,
		open = $bindable(false),
		children
	}: { summary: string | Snippet; meta?: Snippet; open?: boolean; children: Snippet } = $props();
</script>

<details class="disclosure" bind:open>
	<summary>
		<ChevronRight size={14} aria-hidden="true" />
		<span class="summary">
			{#if typeof summary === 'string'}{summary}{:else}{@render summary()}{/if}
		</span>
		{#if meta}<span class="meta">{@render meta()}</span>{/if}
	</summary>
	<div class="content">{@render children()}</div>
</details>

<style>
	summary {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-height: var(--control-md);
		padding: var(--space-1) var(--space-2);
		border-radius: var(--radius-sm);
		color: var(--color-text-secondary);
		cursor: pointer;
		list-style: none;
	}

	summary::-webkit-details-marker {
		display: none;
	}

	summary:hover {
		background: var(--color-surface-hover);
		color: var(--color-text);
	}

	summary > :global(svg) {
		color: var(--color-text-muted);
		transition: transform var(--duration-fast) var(--ease-out);
	}

	details[open] > summary > :global(svg) {
		transform: rotate(90deg);
	}

	.summary {
		flex: 1;
		min-width: 0;
		font-weight: var(--weight-medium);
	}

	.meta {
		display: flex;
		flex: none;
		align-items: center;
		gap: var(--space-2);
		color: var(--color-text-muted);
		font-size: var(--text-sm);
	}

	.content {
		padding: var(--space-2) 0 var(--space-2) var(--space-6);
	}
</style>
