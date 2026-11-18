<script lang="ts">
	import type { Snippet } from 'svelte';
	import { Bot, FolderGit2, Folder } from '@lucide/svelte';
	import { FROM_AGENTS_HINT } from '#lib/prompts/origin.ts';
	import type { ProjectStory } from '#lib/api/types.ts';
	import { integer, money, relativePath } from '#lib/shared/format.ts';
	import { promptListHref, sessionsHref } from '#lib/shared/links.ts';
	import PageHeader from '#lib/ui/PageHeader.svelte';
	import { diffstatText, span } from '#lib/stories/story.ts';

	/** A project's name, folder and time span, then its figures: above every tab of its page. */
	let { story, actions }: { story: ProjectStory; actions?: Snippet } = $props();

	const t = $derived(story.totals);
	const changes = $derived(diffstatText(t.diffstat));
</script>

<PageHeader title={story.name} {actions}>
	{#snippet description()}
		<p class="where">
			{#if story.git}<FolderGit2 size={14} aria-hidden="true" />{:else}<Folder
					size={14}
					aria-hidden="true"
				/>{/if}
			<span class="mono" title={story.repo}>{relativePath(story.repo)}</span>
			<span>{span(t.first_at, t.last_at)}</span>
		</p>
	{/snippet}
</PageHeader>

<section class="summary" aria-label="The project in figures">
	<dl class="figures">
		<div>
			<dt>Prompts you typed</dt>
			<dd>
				<a href={promptListHref({ repo: story.repo })} title="Prompts › {story.name}"
					>{integer(t.prompts)}</a
				>
				{#if story.system}<span class="unit agents" title={FROM_AGENTS_HINT}
						>+ <Bot size={12} aria-hidden="true" />{integer(story.system)} from agents · {integer(
							t.prompts + story.system
						)} in all</span
					>{/if}
			</dd>
		</div>
		<div>
			<dt>Sessions</dt>
			<dd>
				<a href={sessionsHref({ project: story.repo })} title="Sessions › {story.name}"
					>{integer(t.sessions)}</a
				>
			</dd>
		</div>
		<div>
			<dt>Phases</dt>
			<dd>{integer(story.phases.length)}</dd>
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
</section>

<style>
	.where {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1) var(--space-3);
	}

	.where :global(svg) {
		color: var(--color-accent);
	}

	.summary {
		display: grid;
		gap: var(--space-2);
		margin-bottom: var(--space-6);
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

	dd a {
		color: inherit;
	}

	.unit {
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
		font-weight: var(--weight-regular);
	}

	/* Agents' prompts beside yours: the list counts both, this page shows only yours. */
	.agents {
		display: inline-flex;
		align-items: center;
		gap: 3px;
		cursor: help;
	}
</style>
