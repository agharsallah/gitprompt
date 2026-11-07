<script lang="ts">
	import { GitBranch, MessagesSquare, Redo2 } from '@lucide/svelte';
	import { resolve } from '$app/paths';
	import type { StoryEntry } from '#lib/api/types.ts';
	import { timeOfDay } from '#lib/shared/format.ts';
	import RailNode from '#lib/ui/RailNode.svelte';
	import CommitList from '#lib/stories/CommitList.svelte';
	import OutcomeLabel from '#lib/stories/OutcomeLabel.svelte';
	import StoryFacts from '#lib/stories/StoryFacts.svelte';
	import { anchor, railOutcome } from '#lib/stories/story.ts';

	/** A prompt you typed in a project, on the phase's rail, with the commits made after it. */
	let { entry, branch }: { entry: StoryEntry; /** the phase's main branch */ branch?: string } =
		$props();

	const e = $derived(entry);
</script>

<li class="entry" id={anchor(e.id)}>
	<span class="rail"><RailNode outcome={railOutcome(e.outcome)} /></span>
	<div class="body">
		<p class="meta">
			<time datetime={e.received_at}>{timeOfDay(e.received_at)}</time>
			<OutcomeLabel outcome={e.outcome} />
			{#if e.branch && e.branch !== branch}<span class="branch"
					><GitBranch size={12} aria-hidden="true" />{e.branch}</span
				>{/if}
			{#if e.retry_of}<span class="retry" title="Rewords an earlier prompt of its session"
					><Redo2 size={12} aria-hidden="true" />Reworded</span
				>{/if}
			{#if e.session}<a
					class="session"
					href="{resolve('/sessions/[id]', { id: e.session })}#{anchor(e.id)}"
					><MessagesSquare size={12} aria-hidden="true" />In its session</a
				>{/if}
		</p>
		<a class="text clamp-2" href="{resolve('/prompts')}?id={e.id}">{e.text}</a>
		<StoryFacts turn={e} tool />
		{#if e.commits.length}<CommitList commits={e.commits} />{/if}
	</div>
</li>

<style>
	.entry {
		display: grid;
		grid-template-columns: var(--story-rail) minmax(0, 1fr);
		gap: var(--space-3);
		scroll-margin-top: var(--space-6);
	}

	.rail {
		display: flex;
		justify-content: center;
		padding-top: 2px;
	}

	.rail :global(.node) {
		position: relative;
		z-index: 1;
	}

	.body {
		display: grid;
		gap: var(--space-1);
		min-width: 0;
		padding-bottom: var(--space-4);
	}

	.meta {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1) var(--space-3);
		color: var(--color-text-muted);
		font-size: var(--text-xs);
		font-variant-numeric: tabular-nums;
	}

	.branch,
	.retry,
	.session {
		display: inline-flex;
		align-items: center;
		gap: 3px;
	}

	.retry {
		color: var(--color-warning-text);
	}

	.session {
		color: var(--color-text-secondary);
	}

	.text {
		max-width: var(--reading-max);
		color: var(--color-text);
		line-height: var(--leading-snug);
		text-decoration: none;
		overflow-wrap: anywhere;
	}

	.text:hover {
		color: var(--color-accent-text);
		text-decoration: underline;
	}

	.entry:target .text {
		box-shadow: -3px 0 0 var(--color-accent);
	}

	.body > :global(.commits) {
		margin-top: var(--space-1);
	}
</style>
