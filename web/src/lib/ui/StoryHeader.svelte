<script lang="ts">
	import type { Snippet } from 'svelte';
	import PageHeader from '#lib/ui/PageHeader.svelte';

	/**
	 * The top of a story page: its title, a sentence only when something stands out, and the
	 * totals line any period has. Knows nothing of what the figures are: the page passes the words.
	 */
	let {
		title,
		purpose,
		headline,
		totals,
		actions
	}: {
		title: string;
		purpose?: string;
		/** The one figure that stands out, in a sentence; null on an ordinary period. */
		headline?: string | null;
		/** "278 prompts · 41 sessions · $453 · 181 commits". */
		totals: string;
		actions?: Snippet;
	} = $props();
</script>

<PageHeader {title} {purpose} description={headline ?? totals} {actions}>
	{#if headline}<p class="totals">{totals}</p>{/if}
</PageHeader>

<style>
	.totals {
		color: var(--color-text-muted);
		font-size: var(--text-sm);
		font-variant-numeric: tabular-nums;
	}
</style>
