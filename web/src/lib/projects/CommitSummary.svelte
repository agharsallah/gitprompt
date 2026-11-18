<script lang="ts">
	import type { TraceCounts } from '#lib/api/types.ts';
	import { summaryParts, type CommitShow } from '#lib/projects/commits.ts';

	/**
	 * The tab's opening sentence, "212 commits · 187 followed a prompt · 25 by hand": each part is a
	 * filter of the table below, pressed while it is the one applied.
	 */
	let { counts, show = $bindable() }: { counts: TraceCounts; show: CommitShow } = $props();
</script>

<p class="summary" role="group" aria-label="Show commits">
	{#each summaryParts(counts) as part, i (part.show)}
		{#if i}<span class="sep" aria-hidden="true">·</span>{/if}
		<button
			type="button"
			class="part"
			aria-pressed={show === part.show}
			onclick={() => (show = part.show)}>{part.text}</button
		>
	{/each}
</p>

<style>
	.summary {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: var(--space-1) var(--space-2);
		margin: 0 0 var(--space-4);
		font-size: var(--text-lg);
		font-weight: var(--weight-semibold);
		font-stretch: var(--stretch-heading);
		font-variant-numeric: tabular-nums;
	}

	.part {
		padding: 0 var(--space-1);
		border-radius: var(--radius-sm);
		color: var(--color-text-secondary);
		text-decoration: underline dotted var(--color-border-strong);
		text-underline-offset: 0.25em;
	}

	.part:hover {
		color: var(--color-text);
	}

	.part[aria-pressed='true'] {
		color: var(--color-text);
		text-decoration: underline solid var(--color-accent);
	}

	.part:focus-visible {
		box-shadow: var(--focus-ring);
	}

	.sep {
		color: var(--color-text-muted);
	}
</style>
