<script lang="ts">
	import { Search } from '@lucide/svelte';
	import { goto, refreshAll } from '$app/navigation';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import { liveUpdates } from '#lib/api/live.svelte.ts';
	import RangePicker from '#lib/insights/RangePicker.svelte';
	import { rangePhrase } from '#lib/insights/range.ts';
	import { SESSION_PAGE, SORTS, type Sort } from '#lib/sessions/load.ts';
	import SessionCard from '#lib/sessions/SessionCard.svelte';
	import { folder, plural } from '#lib/shared/format.ts';
	import Button from '#lib/ui/Button.svelte';
	import Chip from '#lib/ui/Chip.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import PageHeader from '#lib/ui/PageHeader.svelte';
	import SegmentedControl from '#lib/ui/SegmentedControl.svelte';
	import TextField from '#lib/ui/TextField.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	$effect(() => liveUpdates(refreshAll, 3000));

	/** This page's URL with some parameters changed (null removes one). */
	function url(changes: Record<string, string | null>) {
		const params = new URLSearchParams(page.url.search);
		for (const [key, value] of Object.entries(changes)) {
			if (value === null) params.delete(key);
			else params.set(key, value);
		}
		const query = params.toString();
		return `${resolve('/sessions')}${query ? `?${query}` : ''}`;
	}
	const filtered = $derived(!!(data.q || data.project));
	const more = $derived(data.sessions.length >= data.limit && data.total > data.limit);
	const projectName = (key: string) =>
		data.projects.find((p) => p.key === key)?.detail ?? folder(key) ?? key;
</script>

<svelte:head><title>Sessions · GitPrompt</title></svelte:head>

<PageHeader
	title="Sessions"
	description="Each session is one conversation with the agent: its turns in order, how long each took and how it ended."
>
	{#snippet actions()}<RangePicker
			range={data.filters.range}
			fallback={data.defaultRange}
		/>{/snippet}
</PageHeader>

<div class="controls">
	<form
		class="search"
		method="GET"
		action={resolve('/sessions')}
		data-sveltekit-reset="false"
		role="search"
	>
		{#each ['range', 'project', 'sort'] as key (key)}
			{#if page.url.searchParams.get(key)}<input
					type="hidden"
					name={key}
					value={page.url.searchParams.get(key)}
				/>{/if}
		{/each}
		<TextField
			label="Search sessions"
			hideLabel
			type="search"
			name="q"
			icon={Search}
			value={data.q}
			placeholder="Search what was asked"
			clearable
		/>
	</form>
	<SegmentedControl
		label="Sort sessions"
		value={data.sort as Sort}
		options={(Object.keys(SORTS) as Sort[]).map((s) => ({
			value: s,
			label: SORTS[s].label,
			href: url({ sort: s === 'recent' ? null : s, limit: null })
		}))}
	/>
</div>

{#if data.projects.length > 1 || data.project}
	<div class="projects" role="group" aria-label="Filter by project">
		{#if data.project}
			<Chip
				onremove={() => goto(url({ project: null, limit: null }))}
				removeLabel="Show all projects">{projectName(data.project)}</Chip
			>
		{:else}
			{#each data.projects as p (p.key)}
				<a class="project" href={url({ project: p.key, limit: null })}
					>{p.detail ?? folder(p.key) ?? p.key}</a
				>
			{/each}
		{/if}
	</div>
{/if}

<p class="summary muted" aria-live="polite">
	{#if data.total > data.sessions.length}
		Showing {data.sessions.length} of {plural(data.total, 'session')}{filtered ? ' that match' : ''}
	{:else}
		{plural(data.total, 'session')}{filtered ? ' match' : ''}
	{/if}
	{rangePhrase(data.filters.range)}
</p>

{#if data.sessions.length}
	<div class="list">
		{#each data.sessions as session (session.id)}<SessionCard {session} />{/each}
	</div>
	{#if more}
		<nav class="pager" aria-label="More sessions">
			<Button
				href={url({ limit: String(data.limit + SESSION_PAGE) })}
				size="sm"
				data-sveltekit-noscroll>Show {SESSION_PAGE} more</Button
			>
		</nav>
	{/if}
{:else if filtered}
	<EmptyState title="No sessions match">Try other words, or another project.</EmptyState>
{:else}
	<EmptyState title="No sessions {rangePhrase(data.filters.range)}">
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
		display: inline-flex;
		align-items: center;
		height: 1.5rem;
		padding: 0 var(--space-2);
		border-radius: var(--radius-full);
		box-shadow: inset 0 0 0 1px var(--color-border);
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
		font-weight: var(--weight-medium);
		text-decoration: none;
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

	.pager {
		display: flex;
		justify-content: center;
		margin-top: var(--space-4);
	}
</style>
