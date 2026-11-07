<script lang="ts">
	import type { Origin } from '#lib/api/types.ts';
	import { ORIGINS } from '#lib/prompts/origin.ts';
	import { ALL_ORIGINS } from '#lib/prompts/originFilter.svelte.ts';
	import ToggleChip from '#lib/ui/ToggleChip.svelte';

	/**
	 * The origin filter as checkboxes named `origin` (for a GET form): every origin there are prompts
	 * of, and any selected one; yours first, then the system ones under their own label.
	 */
	let {
		origins,
		counts,
		onchange
	}: {
		/** The origins shown (none = every origin). */
		origins: Origin[];
		/** How many prompts of each origin the other filters keep. */
		counts: Partial<Record<Origin, number>>;
		onchange: () => void;
	} = $props();

	const shown = $derived(ALL_ORIGINS.filter((o) => origins.includes(o) || (counts[o] ?? 0) > 0));
	const yours = $derived(shown.filter((o) => !ORIGINS[o].system));
	const system = $derived(shown.filter((o) => ORIGINS[o].system));
</script>

<fieldset class="origins">
	<legend class="sr-only">Who wrote the prompt</legend>
	{#snippet chip(o: Origin)}
		<ToggleChip
			name="origin"
			value={o}
			checked={origins.includes(o)}
			icon={ORIGINS[o].icon}
			count={counts[o] ?? 0}
			title={ORIGINS[o].hint}
			{onchange}
		>
			{ORIGINS[o].label}
		</ToggleChip>
	{/snippet}
	{#each yours as o (o)}{@render chip(o)}{/each}
	{#if system.length}
		<span class="group-label">System</span>
		{#each system as o (o)}{@render chip(o)}{/each}
	{/if}
</fieldset>

<style>
	fieldset {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
		min-width: 0;
		margin: 0;
		padding: 0;
		border: 0;
	}

	.group-label {
		margin-left: var(--space-1);
		color: var(--color-text-muted);
		font-size: var(--text-xs);
		font-weight: var(--weight-medium);
	}
</style>
