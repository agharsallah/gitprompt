<script lang="ts">
	import type { StoryCommit } from '#lib/api/types.ts';
	import { timeOfDay } from '#lib/shared/format.ts';
	import { commitHref } from '#lib/shared/links.ts';
	import { diffstatText } from '#lib/stories/story.ts';

	/**
	 * Commits made after a prompt: short hash, subject, time and size of the change. The hash links
	 * to the commit on its `remote` (as the daemon reports it), when one is known.
	 */
	let { commits, remote }: { commits: StoryCommit[]; remote?: string } = $props();
</script>

<ul class="commits" aria-label="Commits made after this prompt">
	{#each commits as c (c.hash)}
		{@const href = commitHref(c.hash, remote)}
		<li>
			{#if href}<a {href} title="Commit › {c.hash}"><code class="hash">{c.hash}</code></a
				>{:else}<code class="hash">{c.hash}</code>{/if}
			<span class="subject">{c.subject}</span>
			<span class="meta"
				><time datetime={c.time}>{timeOfDay(c.time)}</time>{#if diffstatText(c)}<span
						>{diffstatText(c)}</span
					>{/if}</span
			>
		</li>
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

	.hash {
		color: var(--color-accent-text);
		font-size: var(--text-xs);
	}

	.subject {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.meta {
		display: inline-flex;
		gap: var(--space-2);
		color: var(--color-text-muted);
		font-size: var(--text-xs);
		font-variant-numeric: tabular-nums;
	}

	@media (max-width: 40rem) {
		li {
			grid-template-columns: auto minmax(0, 1fr);
		}

		.meta {
			grid-column: 2;
		}
	}
</style>
