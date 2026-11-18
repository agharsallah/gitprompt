<script lang="ts">
	import type { StatsQuery } from '#lib/api/query.ts';
	import type { Leaks } from '#lib/api/types.ts';
	import { leaksOfPrompt } from '#lib/leaks/leaks.ts';
	import SecretWarning from '#lib/prompts/SecretWarning.svelte';

	/**
	 * The possible leaks of one turn in the reader: a banner per open finding, a quiet line per value
	 * hidden. Dismissed findings show nothing. Nothing at all while the leaks are on their way.
	 */
	let {
		leaks,
		prompt,
		query = {},
		onchanged,
		now
	}: {
		/** GET /api/leaks over the prompt's session; null while loading or when it failed. */
		leaks: Leaks | null;
		/** The prompt's row id. */
		prompt: number;
		query?: StatsQuery;
		onchanged?: () => void;
		now?: Date;
	} = $props();

	const mine = $derived(leaksOfPrompt(leaks, prompt));
</script>

{#if mine.open.length || mine.hidden.length}
	<div class="leaks">
		{#each mine.open as leak (leak.id)}
			<SecretWarning {leak} {query} {onchanged} {now} />
		{/each}
		{#each mine.hidden as leak (leak.id)}
			<SecretWarning {leak} hidden {now} />
		{/each}
	</div>
{/if}

<style>
	.leaks {
		display: grid;
		gap: var(--space-2);
	}
</style>
