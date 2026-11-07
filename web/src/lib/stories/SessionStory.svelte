<script lang="ts">
	import type { Snippet } from 'svelte';
	import { Bot, FolderGit2, GitBranch } from '@lucide/svelte';
	import { resolve } from '$app/paths';
	import type { Chapter as ChapterData, SessionStory } from '#lib/api/types.ts';
	import { duration, excerpt, integer, modelName, money, plural } from '#lib/shared/format.ts';
	import ConnectorBadge from '#lib/prompts/ConnectorBadge.svelte';
	import Badge from '#lib/ui/Badge.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import PageHeader from '#lib/ui/PageHeader.svelte';
	import Chapter from '#lib/stories/Chapter.svelte';
	import MarkerGroup from '#lib/stories/MarkerGroup.svelte';
	import OutcomeMix from '#lib/stories/OutcomeMix.svelte';
	import { diffstatText, span as timeSpan } from '#lib/stories/story.ts';

	/** A session told as chapters: each prompt you typed, with system activity folded between. */
	let { story, actions }: { story: SessionStory; actions?: Snippet } = $props();

	const chapters = $derived(story.items.filter((i): i is ChapterData => i.kind === 'chapter'));
	const turns = $derived(new Map(chapters.map((c) => [c.id, c.turn])));
	const title = $derived(story.title ?? (excerpt(chapters[0]?.text, 100) || 'Untitled session'));
	const span = $derived(
		story.started_at && story.ended_at
			? (Date.parse(story.ended_at) - Date.parse(story.started_at)) / 1000
			: null
	);
	const t = $derived(story.totals);
	const changes = $derived(diffstatText(t.diffstat));
	const models = $derived(
		Object.entries(story.models ?? {})
			.sort((a, b) => b[1] - a[1])
			.map(([m]) => m)
	);
</script>

<PageHeader {title} {actions}>
	{#snippet description()}
		<p class="where">
			{#if story.connector}<ConnectorBadge connector={story.connector} size="md" />{/if}
			{#each models as m (m)}<Badge tone="agent" icon={Bot} title="Model: {m}">{modelName(m)}</Badge
				>{/each}
			{#if story.project}
				<a class="project" href={resolve('/projects/[id]', { id: story.project.id })}
					><FolderGit2 size={14} aria-hidden="true" />{story.project.name}</a
				>
			{/if}
			{#each story.branches as b (b)}<span class="branch"
					><GitBranch size={13} aria-hidden="true" />{b}</span
				>{/each}
			{#if story.started_at}<time datetime={story.started_at}
					>{timeSpan(story.started_at, story.ended_at)}</time
				>{/if}
		</p>
	{/snippet}
</PageHeader>

<section class="summary" aria-label="The session in figures">
	<dl class="figures">
		<div>
			<dt>Prompts you typed</dt>
			<dd>{integer(t.chapters)}</dd>
		</div>
		<div>
			<dt>System items</dt>
			<dd>{integer(t.markers)}</dd>
		</div>
		<div>
			<dt>Span</dt>
			<dd>{span ? duration(span) : '—'}</dd>
		</div>
		<div>
			<dt>Tool calls</dt>
			<dd>{integer(t.tool_calls)}</dd>
		</div>
		{#if t.diffstat}<div>
				<dt>Commits after your prompts</dt>
				<dd>{integer(t.diffstat.commits)} <span class="unit">{changes}</span></dd>
			</div>{/if}
		{#if t.cost_usd}<div>
				<dt>Cost</dt>
				<dd>{money(t.cost_usd)}</dd>
			</div>{/if}
	</dl>
	{#if t.chapters}
		<div class="mix">
			<OutcomeMix outcomes={t.outcomes} label="How your prompts in this session ended" />
			{#if t.retries}<p class="muted">
					{plural(t.retries, 'prompt')} reworded an earlier one of this session
				</p>{/if}
		</div>
	{/if}
</section>

{#if story.items.length}
	<ol class="story" aria-label="The session, prompt by prompt">
		{#each story.items as item, i (item.kind === 'chapter' ? `c${item.id}` : `m${i}`)}
			{#if item.kind === 'chapter'}
				<Chapter
					chapter={item}
					root={story.project?.repo}
					retryTurn={item.retry_of ? turns.get(item.retry_of) : undefined}
				/>
			{:else}
				<MarkerGroup markers={item.items} />
			{/if}
		{/each}
	</ol>
	{#if !t.chapters}
		<EmptyState compact title="You typed nothing in this session">
			Everything here was sent by the tool or other agents.
		</EmptyState>
	{/if}
{/if}

<style>
	.where {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1) var(--space-4);
	}

	.project,
	.branch {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
	}

	.project {
		color: var(--color-text);
		font-weight: var(--weight-medium);
	}

	.summary {
		display: flex;
		flex-wrap: wrap;
		align-items: flex-end;
		justify-content: space-between;
		gap: var(--space-4) var(--space-8);
		margin-bottom: var(--space-8);
		padding-bottom: var(--space-5);
		border-bottom: var(--border-hairline) solid var(--color-border);
	}

	.figures {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-3) var(--space-6);
		margin: 0;
	}

	.figures div {
		display: grid;
	}

	dt {
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
	}

	dd {
		margin: 0;
		font-size: var(--text-xl);
		font-weight: var(--weight-semibold);
		font-stretch: var(--stretch-heading);
		font-variant-numeric: tabular-nums;
	}

	.unit {
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
		font-weight: var(--weight-regular);
	}

	.mix {
		display: grid;
		gap: var(--space-1);
		width: min(22rem, 100%);
		font-size: var(--text-xs);
	}

	/* The session's rail: one line down the page, through every node. */
	.story {
		--story-rail: 1.5rem;
		position: relative;
		display: grid;
		grid-template-columns: minmax(0, 1fr);
	}

	.story::before {
		content: '';
		position: absolute;
		top: 0.6rem;
		bottom: var(--space-6);
		left: calc(var(--story-rail) / 2 - 1px);
		width: 2px;
		background: var(--color-border-strong);
	}
</style>
