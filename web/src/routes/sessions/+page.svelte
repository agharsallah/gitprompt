<script lang="ts">
	import { Search } from '@lucide/svelte';
	import { refreshAll } from '$app/navigation';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import { liveUpdates } from '#lib/api/live.svelte.ts';
	import { sessions } from '#lib/insights/aggregate.ts';
	import RangePicker from '#lib/insights/RangePicker.svelte';
	import { rangePhrase } from '#lib/insights/range.ts';
	import { plural } from '#lib/shared/format.ts';
	import SessionCard from '#lib/sessions/SessionCard.svelte';
	import Chip from '#lib/ui/Chip.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import PageHeader from '#lib/ui/PageHeader.svelte';
	import SegmentedControl from '#lib/ui/SegmentedControl.svelte';
	import TextField from '#lib/ui/TextField.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	$effect(() => liveUpdates(refreshAll, 3000));

	const all = $derived(sessions(data.window.prompts));
	let query = $state('');
	// Starts from ?project= (links from the Projects page), then is local state.
	let project = $state<string | null>(page.url.searchParams.get('project'));
	let sort = $state<'recent' | 'longest' | 'costliest'>('recent');

	const projectNames = $derived(
		[...new Map(all.map((s) => [s.project.key, s.project.name]))].sort((a, b) => a[1].localeCompare(b[1]))
	);
	const shown = $derived.by(() => {
		const q = query.trim().toLowerCase();
		const list = all.filter(
			(s) =>
				(!project || s.project.key === project) &&
				(!q ||
					s.title?.toLowerCase().includes(q) ||
					s.prompts.some((p) => p.prompt?.toLowerCase().includes(q)))
		);
		if (sort === 'longest') return [...list].sort((a, b) => b.prompts.length - a.prompts.length);
		if (sort === 'costliest') return [...list].sort((a, b) => b.cost - a.cost);
		return list;
	});
</script>

<svelte:head><title>Sessions · GitPrompt</title></svelte:head>

<PageHeader
	title="Sessions"
	description="Each session is one conversation with the agent: its turns in order, how long each took and how it ended."
>
	{#snippet actions()}<RangePicker range={data.range} />{/snippet}
</PageHeader>

<div class="controls">
	<div class="search">
		<TextField label="Search sessions" hideLabel type="search" icon={Search} bind:value={query} placeholder="Search what was asked" clearable />
	</div>
	<SegmentedControl
		label="Sort sessions"
		bind:value={sort}
		options={[
			{ value: 'recent', label: 'Recent' },
			{ value: 'longest', label: 'Most turns' },
			{ value: 'costliest', label: 'Costliest' }
		]}
	/>
</div>

{#if projectNames.length > 1}
	<div class="projects" role="group" aria-label="Filter by project">
		{#each projectNames as [key, name] (key)}
			{#if project === key}
				<Chip onremove={() => (project = null)} removeLabel="Show all projects">{name}</Chip>
			{:else}
				<button class="project" type="button" onclick={() => (project = key)}>{name}</button>
			{/if}
		{/each}
	</div>
{/if}

<p class="summary muted" aria-live="polite">
	{plural(shown.length, 'session')}{project || query ? ' match' : ''} {rangePhrase(data.range)}
</p>

{#if shown.length}
	<div class="list">
		{#each shown as session (session.id)}<SessionCard {session} />{/each}
	</div>
{:else if all.length}
	<EmptyState title="No sessions match">Try other words, or another project.</EmptyState>
{:else}
	<EmptyState title="No sessions {rangePhrase(data.range)}">
		Pick a longer range, or <a href={resolve('/sources')}>check that a source is connected</a>.
	</EmptyState>
{/if}

<style>
	.controls {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-3);
		margin-bottom: var(--space-3);
	}

	.search {
		flex: 1 1 18rem;
		max-width: 32rem;
	}

	.projects {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1_5);
		margin-bottom: var(--space-4);
	}

	.project {
		height: 1.5rem;
		padding: 0 var(--space-2);
		border-radius: var(--radius-full);
		box-shadow: inset 0 0 0 1px var(--color-border);
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
		font-weight: var(--weight-medium);
	}

	.project:hover {
		box-shadow: inset 0 0 0 1px var(--color-border-control);
		color: var(--color-text);
	}

	.summary {
		margin-bottom: var(--space-3);
		font-size: var(--text-sm);
	}

	.list {
		display: grid;
		gap: var(--space-3);
	}
</style>
