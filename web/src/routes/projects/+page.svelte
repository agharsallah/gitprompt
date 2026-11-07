<script lang="ts">
	import { refreshAll } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { liveUpdates } from '#lib/api/live.svelte.ts';
	import RangePicker from '#lib/insights/RangePicker.svelte';
	import { rangePhrase, rangeStart } from '#lib/insights/range.ts';
	import { chartStart, timeSlots } from '#lib/insights/series.ts';
	import ProjectRow from '#lib/projects/ProjectRow.svelte';
	import { projectSummaries } from '#lib/projects/summary.ts';
	import { plural } from '#lib/shared/format.ts';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import PageHeader from '#lib/ui/PageHeader.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	$effect(() => liveUpdates(refreshAll, 3000));

	const totals = $derived(data.byDay.totals);
	const slots = $derived(
		timeSlots(
			[],
			chartStart(rangeStart(data.filters.range, data.now), totals.first_at, data.now),
			new Date(data.now),
			false
		)
	);
	const list = $derived(
		projectSummaries(data.byDay.groups, data.byKind.groups, data.byBranch.groups, slots)
	);
</script>

<svelte:head><title>Projects · GitPrompt</title></svelte:head>

<PageHeader
	title="Projects"
	description="Where you worked with the agent: each repository or folder you sent prompts from, busiest first."
>
	{#snippet actions()}<RangePicker
			range={data.filters.range}
			fallback={data.defaultRange}
		/>{/snippet}
</PageHeader>

<p class="summary muted">{plural(list.length, 'project')} {rangePhrase(data.filters.range)}</p>

{#if list.length}
	<div class="list">
		{#each list as summary (summary.key)}
			<ProjectRow {summary} share={summary.prompts / totals.prompts} />
		{/each}
	</div>
{:else}
	<EmptyState title="No projects {rangePhrase(data.filters.range)}">
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
