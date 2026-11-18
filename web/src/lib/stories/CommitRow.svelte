<script lang="ts">
	import type { StoryCommit } from '#lib/api/types.ts';
	import { timeOfDay } from '#lib/shared/format.ts';
	import { diffstatText } from '#lib/stories/story.ts';

	/**
	 * One commit on one line, as three cells the parent lays out (hash, subject, meta): the short
	 * hash, linked to `href` (its page on the forge, as the daemon gives it) in a new tab when there
	 * is one, the subject, then when it was made (`time`: the time of day unless given) and the size
	 * of the change.
	 */
	let {
		commit,
		href,
		time = timeOfDay
	}: { commit: StoryCommit; href?: string; time?: (iso: string) => string } = $props();

	const change = $derived(diffstatText(commit));
</script>

{#if href}<a {href} target="_blank" rel="noopener" title="Commit › {commit.hash}"
		><code class="hash">{commit.hash}</code></a
	>{:else}<code class="hash">{commit.hash}</code>{/if}
<span class="subject">{commit.subject}</span>
<span class="meta"
	><time datetime={commit.time}>{time(commit.time)}</time>{#if change}<span>{change}</span
		>{/if}</span
>

<style>
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
</style>
