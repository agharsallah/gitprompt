<script lang="ts">
	import { X, type LucideIcon } from '@lucide/svelte';
	import type { Snippet } from 'svelte';

	/**
	 * A tag-like token. With `href` it is a link (e.g. "filter by this tag"); with `onremove` it gets
	 * a remove button (e.g. an active filter, a tag being edited).
	 */
	let {
		href,
		onremove,
		removeLabel = 'Remove',
		icon: Icon,
		mono = false,
		children
	}: {
		href?: string;
		onremove?: () => void;
		removeLabel?: string;
		icon?: LucideIcon;
		mono?: boolean;
		children: Snippet;
	} = $props();
</script>

<span class={['chip', mono && 'mono-chip']}>
	{#if href}
		<a class="body" {href}>
			{#if Icon}<Icon size={12} aria-hidden="true" />{/if}{@render children()}
		</a>
	{:else}
		<span class="body">
			{#if Icon}<Icon size={12} aria-hidden="true" />{/if}{@render children()}
		</span>
	{/if}
	{#if onremove}
		<button class="remove" type="button" onclick={onremove} aria-label={removeLabel} title={removeLabel}>
			<X size={12} aria-hidden="true" />
		</button>
	{/if}
</span>

<style>
	.chip {
		display: inline-flex;
		align-items: center;
		max-width: 100%;
		height: 1.5rem;
		border: var(--border-hairline) solid var(--color-border-strong);
		border-radius: var(--radius-full);
		background: var(--color-surface-raised);
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
		font-weight: var(--weight-medium);
		line-height: 1;
	}

	.mono-chip {
		font-family: var(--font-mono);
		font-weight: var(--weight-regular);
	}

	.body {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
		height: 100%;
		padding: 0 var(--space-2);
		overflow: hidden;
		color: inherit;
		text-decoration: none;
		white-space: nowrap;
		text-overflow: ellipsis;
		border-radius: var(--radius-full);
	}

	a.body:hover {
		color: var(--color-accent-text);
	}

	.remove {
		display: grid;
		place-items: center;
		width: 1.25rem;
		height: 1.25rem;
		margin-right: 2px;
		margin-left: calc(-1 * var(--space-1));
		border-radius: 50%;
		color: var(--color-text-muted);
	}

	.remove:hover {
		background: var(--color-surface-hover);
		color: var(--color-text);
	}
</style>
