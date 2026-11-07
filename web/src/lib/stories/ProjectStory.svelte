<script lang="ts">
	import type { Snippet } from 'svelte';
	import { FolderGit2, Folder } from '@lucide/svelte';
	import { resolve } from '$app/paths';
	import type { ProjectStory } from '#lib/api/types.ts';
	import { integer, money, plural, relativePath } from '#lib/shared/format.ts';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import PageHeader from '#lib/ui/PageHeader.svelte';
	import Evolution from '#lib/stories/Evolution.svelte';
	import PhaseSection from '#lib/stories/PhaseSection.svelte';
	import { diffstatText, span } from '#lib/stories/story.ts';

	/** How a project got built, prompt by prompt: its phases, then each phase on a rail. */
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
				<a href="{resolve('/prompts')}?repo={encodeURIComponent(story.repo)}"
					>{integer(t.prompts)}</a
				>
			</dd>
		</div>
		<div>
			<dt>Sessions</dt>
			<dd>
				<a href="{resolve('/sessions')}?project={encodeURIComponent(story.repo)}"
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
	{#if story.system}<p class="muted note">
			Not shown: {plural(story.system, 'prompt')} sent by the tool or other agents here.
		</p>{/if}
</section>

{#if story.phases.length}
	{#if story.phases.length > 1}<Evolution phases={story.phases} />{/if}
	<div class="phases">
		{#each story.phases as phase (phase.index)}<PhaseSection {phase} />{/each}
	</div>
{:else}
	<EmptyState title="You typed nothing in this project">
		Every prompt here was sent by the tool or other agents.
	</EmptyState>
{/if}

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

	.note {
		font-size: var(--text-sm);
	}

	.phases {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		gap: var(--space-10);
		margin-top: var(--space-8);
	}
</style>
