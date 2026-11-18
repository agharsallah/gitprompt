<script lang="ts">
	import type { Snippet } from 'svelte';
	import PageHeader from '#lib/ui/PageHeader.svelte';

	/**
	 * The top of a story page: its title, a sentence only when something stands out, the totals
	 * line any period has, and the figures (StatTiles) under it. Knows nothing of what the figures
	 * are: the page passes the words and the tiles.
	 */
	let {
		title,
		purpose,
		headline,
		totals,
		actions,
		figures
	}: {
		title: string;
		purpose?: string;
		/** The one figure that stands out, in a sentence; null on an ordinary period. */
		headline?: string | null;
		/** "278 prompts · 41 sessions · $453 · 181 commits". */
		totals: string;
		actions?: Snippet;
		/** The figure tiles, laid out in a row under the header. */
		figures?: Snippet;
	} = $props();
</script>

<PageHeader {title} {purpose} description={headline ?? totals} {actions}>
	{#if headline}<p class="totals">{totals}</p>{/if}
</PageHeader>
{#if figures}<div class="figures">{@render figures()}</div>{/if}

<style>
	.totals {
		color: var(--color-text-muted);
		font-size: var(--text-sm);
		font-variant-numeric: tabular-nums;
	}

	.figures {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(100%, 12rem), 1fr));
		gap: var(--space-3);
		margin-bottom: var(--space-6);
	}
</style>
