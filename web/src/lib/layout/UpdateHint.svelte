<script lang="ts">
	import { CircleFadingArrowUp, X } from '@lucide/svelte';
	import { fly } from 'svelte/transition';
	import { resolve } from '$app/paths';
	import { updates } from '#lib/layout/updates.svelte.ts';
	import { updateQuery, updateSummary } from '#lib/sources/updates.ts';

	/**
	 * A quiet note at the bottom of the sidebar when a connection needs an update: it never covers
	 * the page and never interrupts, and "Later" hides it until another connection needs one.
	 * The Connectors page says the same on the card itself, so the sidebar hides it there.
	 */
	let { hidden = false }: { hidden?: boolean } = $props();
	const visible = $derived(!hidden && updates.list.length > 0 && !updates.dismissed);
	const reduced =
		typeof matchMedia !== 'undefined' && matchMedia('(prefers-reduced-motion: reduce)').matches;
</script>

{#if visible}
	<div class="hint" role="status" transition:fly={{ y: 6, duration: reduced ? 0 : 180 }}>
		<CircleFadingArrowUp size={16} aria-hidden="true" class="icon" />
		<div class="body">
			<p class="title">Update available</p>
			<p class="text">{updateSummary(updates.list)}</p>
			<a class="review" href="{resolve('/sources')}{updateQuery(updates.list)}">
				{updates.list.length === 1 ? 'Review the update' : 'Review updates'}
			</a>
		</div>
		<button
			type="button"
			class="later"
			aria-label="Hide until another connection needs an update"
			title="Later"
			onclick={() => updates.dismiss()}
		>
			<X size={14} aria-hidden="true" />
		</button>
	</div>
{/if}

<style>
	.hint {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr) auto;
		gap: var(--space-2);
		padding: var(--space-2) var(--space-2) var(--space-2) var(--space-3);
		border: var(--border-hairline) solid var(--color-info-border);
		border-radius: var(--radius-md);
		background: var(--color-info-soft);
		font-size: var(--text-xs);
		line-height: var(--leading-snug);
	}

	.hint :global(.icon) {
		margin-top: 1px;
		color: var(--color-info-solid);
	}

	.body {
		display: grid;
		gap: 2px;
		min-width: 0;
	}

	.title {
		color: var(--color-text);
		font-weight: var(--weight-medium);
	}

	.text {
		color: var(--color-text-secondary);
	}

	.review {
		justify-self: start;
		margin-top: var(--space-1);
		color: var(--color-info-text);
		font-weight: var(--weight-medium);
	}

	.later {
		display: grid;
		place-items: center;
		width: 22px;
		height: 22px;
		border: 0;
		border-radius: var(--radius-sm);
		background: none;
		color: var(--color-text-muted);
		cursor: pointer;
	}

	.later:hover {
		background: color-mix(in srgb, var(--color-info-solid) 12%, transparent);
		color: var(--color-text);
	}
</style>
