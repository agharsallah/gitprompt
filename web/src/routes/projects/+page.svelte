<script lang="ts">
	import { refreshAll } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { liveUpdates } from '#lib/api/live.svelte.ts';
	import { byDay, projects } from '#lib/insights/aggregate.ts';
	import RangePicker from '#lib/insights/RangePicker.svelte';
	import { rangePhrase } from '#lib/insights/range.ts';
	import { plural } from '#lib/shared/format.ts';
	import ProjectRow from '#lib/projects/ProjectRow.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import PageHeader from '#lib/ui/PageHeader.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	$effect(() => liveUpdates(refreshAll, 3000));

	const prompts = $derived(data.window.prompts);
	const start = $derived(
		data.window.start ?? new Date(prompts.length ? Date.parse(prompts.at(-1)!.received_at) : Date.now())
	);
	const list = $derived(projects(prompts, byDay(prompts, start, new Date(data.window.loadedAt))));
</script>

<svelte:head><title>Projects · GitPrompt</title></svelte:head>

<PageHeader
	title="Projects"
	description="Where you worked with the agent: each repository or folder you sent prompts from, busiest first."
>
	{#snippet actions()}<RangePicker range={data.range} />{/snippet}
</PageHeader>

<p class="summary muted">{plural(list.length, 'project')} {rangePhrase(data.range)}</p>

{#if list.length}
	<div class="list">
		{#each list as summary (summary.project.key)}
			<ProjectRow {summary} share={summary.prompts / prompts.length} />
		{/each}
	</div>
{:else}
	<EmptyState title="No projects {rangePhrase(data.range)}">
		Projects appear as soon as you send a prompt from a folder. Pick a longer range, or
		<a href={resolve('/sources')}>check your sources</a>.
	</EmptyState>
{/if}

<style>
	.summary {
		margin-bottom: var(--space-3);
		font-size: var(--text-sm);
	}

	.list {
		display: grid;
		gap: var(--space-3);
	}
</style>
