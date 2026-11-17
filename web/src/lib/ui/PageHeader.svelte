<script lang="ts">
	import type { Snippet } from 'svelte';

	/** The top of every page: what it is, one line on what it is for, and its page-wide controls. */
	let {
		title,
		heading,
		purpose,
		description,
		actions,
		children
	}: {
		title: string;
		/** Draws the heading's content itself (one you can rename); `title` stays its plain text. */
		heading?: Snippet;
		/** The job the page serves, in a few words (layout/nav.ts), shown right under the title. */
		purpose?: string;
		description?: string | Snippet;
		actions?: Snippet;
		/** Extra content under the description (summary chips, a status line). */
		children?: Snippet;
	} = $props();
</script>

<header class="page-header">
	<div class="text">
		<h1>
			{#if heading}{@render heading()}{:else}{title}{/if}
		</h1>
		{#if purpose}<p class="purpose">{purpose}</p>{/if}
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

	.purpose {
		color: var(--color-accent-text);
		font-size: var(--text-sm);
		font-weight: var(--weight-medium);
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
