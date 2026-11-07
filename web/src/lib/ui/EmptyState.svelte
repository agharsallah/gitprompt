<script lang="ts">
	import type { Snippet } from 'svelte';

	/** What to do when there is nothing here yet. Drawn with the rail: two nodes and an open one. */
	let {
		title,
		children,
		actions,
		compact = false
	}: { title: string; children?: Snippet; actions?: Snippet; compact?: boolean } = $props();
</script>

<div class={['empty', compact && 'compact']}>
	{#if !compact}
		<svg class="rail" viewBox="0 0 120 24" aria-hidden="true">
			<line x1="8" y1="12" x2="112" y2="12" />
			<circle class="filled" cx="12" cy="12" r="5" />
			<circle class="filled" cx="60" cy="12" r="5" />
			<circle class="open" cx="108" cy="12" r="5" />
		</svg>
	{/if}
	<p class="title">{title}</p>
	{#if children}<div class="body">{@render children()}</div>{/if}
	{#if actions}<div class="actions">{@render actions()}</div>{/if}
</div>

<style>
	.empty {
		display: grid;
		justify-items: center;
		gap: var(--space-2);
		padding: var(--space-12) var(--space-6);
		text-align: center;
	}

	.compact {
		justify-items: start;
		padding: var(--space-4) 0;
		text-align: left;
	}

	.rail {
		width: 7.5rem;
		margin-bottom: var(--space-2);
	}

	line {
		stroke: var(--color-border-strong);
		stroke-width: 2;
	}

	.filled {
		fill: var(--color-accent);
		stroke: var(--color-canvas);
		stroke-width: 2;
	}

	.open {
		fill: var(--color-canvas);
		stroke: var(--color-accent);
		stroke-width: 2;
	}

	.title {
		font-size: var(--text-lg);
		font-weight: var(--weight-semibold);
	}

	.compact .title {
		font-size: var(--text-md);
	}

	.body {
		max-width: 46ch;
		color: var(--color-text-secondary);
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		justify-content: center;
		gap: var(--space-2);
		margin-top: var(--space-2);
	}
</style>
