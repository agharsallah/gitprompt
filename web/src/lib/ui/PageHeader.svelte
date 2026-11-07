<script lang="ts">
	import type { Snippet } from 'svelte';

	/** The top of every page: what it is, one line on what it is for, and its page-wide controls. */
	let {
		title,
		description,
		actions,
		children
	}: {
		title: string;
		description?: string | Snippet;
		actions?: Snippet;
		/** Extra content under the description (summary chips, a status line). */
		children?: Snippet;
	} = $props();
</script>

<header class="page-header">
	<div class="text">
		<h1>{title}</h1>
		{#if typeof description === 'string'}
			<p class="description">{description}</p>
		{:else if description}
			<div class="description">{@render description()}</div>
		{/if}
		{@render children?.()}
	</div>
	{#if actions}<div class="actions">{@render actions()}</div>{/if}
</header>

<style>
	.page-header {
		display: flex;
		flex-wrap: wrap;
		align-items: flex-end;
		justify-content: space-between;
		gap: var(--space-4) var(--space-6);
		margin-bottom: var(--space-6);
	}

	.text {
		display: grid;
		gap: var(--space-2);
		min-width: min(100%, 20rem);
		flex: 1;
	}

	.description {
		max-width: var(--reading-max);
		color: var(--color-text-secondary);
		font-size: var(--text-lg);
		line-height: var(--leading-snug);
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
	}
</style>
