<script lang="ts">
	import { GitBranch, Redo2 } from '@lucide/svelte';
	import { resolve } from '$app/paths';
	import type { Chapter } from '#lib/api/types.ts';
	import { plural, relativePath, timeOfDay } from '#lib/shared/format.ts';
	import Disclosure from '#lib/ui/Disclosure.svelte';
	import RailNode from '#lib/ui/RailNode.svelte';
	import CommitList from '#lib/stories/CommitList.svelte';
	import OutcomeLabel from '#lib/stories/OutcomeLabel.svelte';
	import StoryFacts from '#lib/stories/StoryFacts.svelte';
	import ToolMix from '#lib/stories/ToolMix.svelte';
	import { anchor, railOutcome } from '#lib/stories/story.ts';

	/**
	 * One prompt you typed, as a chapter of its session: on the rail (the node's shape is the
	 * outcome), your words, then what the turn did: time, tools, files, commits, cost.
	 */
	let {
		chapter,
		root,
		retryTurn
	}: {
		chapter: Chapter;
		/** The repository root, to show file paths relative to it. */
		root?: string | null;
		/** The turn number of the chapter this one rewords. */
		retryTurn?: number;
	} = $props();

	const c = $derived(chapter);
	const href = $derived(`${resolve('/prompts')}?id=${c.id}`);
	const long = $derived(c.text.length > 700 || c.text.split('\n').length > 10);
	let expanded = $state(false);
</script>

<li class="chapter" id={anchor(c.id)} aria-labelledby="{anchor(c.id)}-title">
	<span class="rail"><RailNode outcome={railOutcome(c.outcome)} size={16} /></span>
	<div class="body">
		<p class="meta">
			<a id="{anchor(c.id)}-title" class="turn" {href}>Turn {c.turn}</a>
			<time datetime={c.received_at}>{timeOfDay(c.received_at)}</time>
			<OutcomeLabel outcome={c.outcome} />
			{#if c.branch}<span class="branch"><GitBranch size={12} aria-hidden="true" />{c.branch}</span
				>{/if}
			{#if c.retry_of}
				<a class="retry" href="#{anchor(c.retry_of)}"
					><Redo2 size={12} aria-hidden="true" />Rewords turn {retryTurn ?? ''}</a
				>
			{/if}
		</p>
		<p class={['said', long && !expanded && 'folded']}>{c.text}</p>
		{#if long || c.clipped}
			<p class="more">
				{#if long}<button
						type="button"
						aria-expanded={expanded}
						onclick={() => (expanded = !expanded)}
						>{expanded ? 'Show less' : 'Show the whole prompt'}</button
					>{/if}
				{#if c.clipped}<a {href}>Read it all in the prompt</a>{/if}
			</p>
		{/if}
		<div class="facts">
			<StoryFacts turn={c} />
			<ToolMix tools={c.tools} label="Tool calls of turn {c.turn} by kind" />
		</div>
		{#if c.files.length}
			<Disclosure summary={plural(c.files_written, 'file') + ' written'}>
				<ul class="files">
					{#each c.files as f (f)}<li class="mono">{relativePath(f, root)}</li>{/each}
					{#if c.files_written > c.files.length}<li class="muted">
							and {c.files_written - c.files.length} more
						</li>{/if}
				</ul>
			</Disclosure>
		{/if}
		{#if c.commits.length}<CommitList commits={c.commits} />{/if}
	</div>
</li>

<style>
	.chapter {
		display: grid;
		grid-template-columns: var(--story-rail) minmax(0, 1fr);
		gap: var(--space-3);
		scroll-margin-top: var(--space-6);
	}

	.rail {
		display: flex;
		justify-content: center;
		padding-top: 3px;
	}

	.rail :global(.node) {
		position: relative;
		z-index: 1;
	}

	.body {
		display: grid;
		gap: var(--space-2);
		min-width: 0;
		padding-bottom: var(--space-6);
	}

	.chapter:target .said {
		box-shadow: -3px 0 0 var(--color-accent);
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

	.turn {
		color: var(--color-text);
		font-size: var(--text-sm);
		font-weight: var(--weight-semibold);
		text-decoration: none;
	}

	.turn:hover {
		color: var(--color-accent-text);
		text-decoration: underline;
	}

	.branch,
	.retry {
		display: inline-flex;
		align-items: center;
		gap: 3px;
	}

	.retry {
		color: var(--color-warning-text);
	}

	.said {
		max-width: var(--reading-max);
		padding-left: var(--space-3);
		color: var(--color-text);
		font-size: var(--text-lg);
		font-weight: var(--weight-medium);
		line-height: var(--leading-relaxed);
		white-space: pre-wrap;
		overflow-wrap: anywhere;
		transition: box-shadow var(--duration-base) var(--ease-out);
	}

	.folded {
		max-height: 12rem;
		overflow: hidden;
		mask-image: linear-gradient(to bottom, #000 70%, transparent);
	}

	.more {
		display: flex;
		gap: var(--space-4);
		padding-left: var(--space-3);
		font-size: var(--text-sm);
	}

	.more button {
		color: var(--color-accent-text);
		font-weight: var(--weight-medium);
	}

	.more button:hover {
		text-decoration: underline;
	}

	.facts {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2) var(--space-5);
		padding-left: var(--space-3);
	}

	.body > :global(.disclosure),
	.body > :global(.commits) {
		margin-left: var(--space-1);
		font-size: var(--text-sm);
	}

	.body > :global(.commits) {
		padding-left: var(--space-2);
	}

	.files {
		display: grid;
		gap: 2px;
		font-size: var(--text-xs);
		overflow-wrap: anywhere;
	}
</style>
