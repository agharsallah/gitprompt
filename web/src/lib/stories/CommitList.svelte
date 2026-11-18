<script lang="ts">
	import type { StoryCommit } from '#lib/api/types.ts';
	import CommitRow from '#lib/stories/CommitRow.svelte';

	/** Commits made after a prompt: short hash, subject, time and size of the change. */
	let { commits }: { commits: StoryCommit[] } = $props();
</script>

<ul class="commits" aria-label="Commits made after this prompt">
	{#each commits as c (c.hash)}
		<li><CommitRow commit={c} /></li>
	{/each}
</ul>

<style>
	.commits {
		display: grid;
		gap: var(--space-1);
		font-size: var(--text-sm);
	}

	li {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr) auto;
		align-items: baseline;
		gap: var(--space-2);
	}

	@media (max-width: 40rem) {
		li {
			grid-template-columns: auto minmax(0, 1fr);
		}

		li > :global(.meta) {
			grid-column: 2;
		}
	}
</style>
