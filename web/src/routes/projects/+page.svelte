<script lang="ts">
	import { refreshAll } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { liveUpdates } from '#lib/api/live.svelte.ts';
	import Deferred from '#lib/async/Deferred.svelte';
	import { Resource } from '#lib/async/resource.svelte.ts';
	import RangePicker from '#lib/insights/RangePicker.svelte';
	import { rangePhrase, rangeStart } from '#lib/insights/range.ts';
	import { chartStart, timeSlots } from '#lib/insights/series.ts';
	import ProjectRow from '#lib/projects/ProjectRow.svelte';
	import { projectSummaries } from '#lib/projects/summary.ts';
	import { plural } from '#lib/shared/format.ts';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import ListSkeleton from '#lib/ui/ListSkeleton.svelte';
	import PageHeader from '#lib/ui/PageHeader.svelte';
	import Skeleton from '#lib/ui/Skeleton.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	$effect(() => liveUpdates(refreshAll, 3000));

	const stats = new Resource(() => data.stats);
	const summarize = ({ byDay, byKind, byBranch }: NonNullable<typeof stats.value>) => {
		const start = chartStart(
			rangeStart(data.filters.range, data.now),
			byDay.totals.first_at,
			data.now
		);
		const slots = timeSlots([], start, new Date(data.now), false);
		return projectSummaries(byDay.groups, byKind.groups, byBranch.groups, slots);
	};
</script>

<svelte:head><title>Projects · GitPrompt</title></svelte:head>

<PageHeader
	title="Projects"
	description="Where you worked with the agent: each repository or folder you sent prompts from, busiest first."
>
	{#snippet actions()}<RangePicker range={data.filters.range} />{/snippet}
</PageHeader>

<Deferred resource={stats} label="Loading projects" what="the projects">
	{#snippet pending()}
		<p class="summary"><Skeleton width="8rem" height="0.8rem" /></p>
		<ListSkeleton rows={4} height="2rem" />
	{/snippet}
	{#snippet children(s)}
		{@const list = summarize(s)}
		<p class="summary muted">{plural(list.length, 'project')} {rangePhrase(data.filters.range)}</p>

		{#if list.length}
			<div class="list">
				{#each list as summary (summary.key)}
					<ProjectRow {summary} share={summary.prompts / s.byDay.totals.prompts} />
				{/each}
			</div>
		{:else}
			<EmptyState title="No projects {rangePhrase(data.filters.range)}">
				Projects appear as soon as you send a prompt from a folder. Pick a longer range, or
				<a href={resolve('/sources')}>check your sources</a>.
			</EmptyState>
		{/if}
	{/snippet}
</Deferred>

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
