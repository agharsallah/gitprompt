<script lang="ts">
	import type { Snippet } from 'svelte';

	/**
	 * A titled region of a page. `tone` sets its weight in the hierarchy: outlined panels hold the
	 * main content, sunken ones hold secondary detail, plain ones are just a heading and content.
	 */
	let {
		title,
		description,
		level = 2,
		tone = 'outlined',
		pad = 'md',
		actions,
		footer,
		children,
		class: className
	}: {
		title?: string;
		description?: string | Snippet;
		level?: 2 | 3;
		tone?: 'outlined' | 'sunken' | 'plain';
		pad?: 'none' | 'sm' | 'md';
		actions?: Snippet;
		footer?: Snippet;
		children: Snippet;
		class?: string;
	} = $props();

	const id = $props.id();
</script>

<section class={['panel', `tone-${tone}`, `pad-${pad}`, className]} aria-labelledby={title ? id : undefined}>
	{#if title || actions}
		<header>
			<div class="titles">
				{#if title}
					<svelte:element this={`h${level}`} {id} class="title">{title}</svelte:element>
				{/if}
				{#if typeof description === 'string'}
					<p class="description">{description}</p>
				{:else if description}
					<div class="description">{@render description()}</div>
				{/if}
			</div>
			{#if actions}<div class="actions">{@render actions()}</div>{/if}
		</header>
	{/if}
	<div class="body">{@render children()}</div>
	{#if footer}<footer>{@render footer()}</footer>{/if}
</section>

<style>
	.panel {
		--pad: var(--space-5);
		display: flex;
		flex-direction: column;
		min-width: 0;
		border-radius: var(--radius-lg);
	}

	.pad-sm {
		--pad: var(--space-4);
	}

	.pad-none {
		--pad: 0;
	}

	.tone-outlined {
		padding: var(--pad);
		background: var(--color-surface);
		box-shadow: inset 0 0 0 1px var(--color-border);
	}

	.tone-sunken {
		padding: var(--pad);
		background: var(--color-surface-sunken);
	}

	header {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: var(--space-3);
		margin-bottom: var(--space-4);
	}

	.pad-none header {
		padding: var(--space-4) var(--space-4) 0;
	}

	.titles {
		display: grid;
		gap: var(--space-1);
		min-width: 0;
	}

	.title {
		font-size: var(--text-lg);
	}

	h3.title {
		font-size: var(--text-md);
	}

	.description {
		max-width: var(--reading-max);
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}

	.actions {
		display: flex;
		flex: none;
		align-items: center;
		gap: var(--space-2);
	}

	.body {
		flex: 1;
		min-width: 0;
	}

	footer {
		margin-top: var(--space-4);
		padding-top: var(--space-3);
		border-top: var(--border-hairline) solid var(--color-border);
		color: var(--color-text-muted);
		font-size: var(--text-sm);
	}
</style>
