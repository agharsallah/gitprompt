<script lang="ts">
	import { onMount } from 'svelte';
	import { refreshAll, replaceState } from '$app/navigation';
	import { page } from '$app/state';
	import { liveUpdates } from '#lib/api/live.svelte.ts';
	import Deferred from '#lib/async/Deferred.svelte';
	import { Resource } from '#lib/async/resource.svelte.ts';
	import Boundary from '#lib/errors/Boundary.svelte';
	import { tools } from '#lib/sources/state.ts';
	import RecentCaptures from '#lib/sources/RecentCaptures.svelte';
	import SpecProblems from '#lib/sources/SpecProblems.svelte';
	import ToolCard from '#lib/sources/ToolCard.svelte';
	import ListSkeleton from '#lib/ui/ListSkeleton.svelte';
	import PageHeader from '#lib/ui/PageHeader.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	$effect(() => liveUpdates(refreshAll, 2000));

	const connectors = new Resource(() => data.connectors);
	// First run: /sources?connect=<id> opens that connector's consent review straight away. Read
	// once, then dropped from the URL, so a reload doesn't reopen it.
	const review = page.url.searchParams.get('connect');
	onMount(() => {
		if (!review) return;
		const url = new URL(page.url.href);
		url.searchParams.delete('connect');
		void replaceState(url, page.state);
	});
</script>

<svelte:head><title>Connectors · GitPrompt</title></svelte:head>

<PageHeader
	title="Connectors"
	description="The tools GitPrompt captures from. It only uses each tool's documented extension points, and you review the exact change to its settings before anything is written."
/>

<Deferred resource={connectors} label="Loading connectors" what="the connectors">
	{#snippet pending()}
		<div class="tools">
			<ListSkeleton rows={2} height="9rem" /><ListSkeleton rows={2} height="9rem" />
		</div>
	{/snippet}
	{#snippet children({ sources, origins, problems, recent })}
		{#if problems.length}
			<div class="problems"><SpecProblems {problems} /></div>
		{/if}

		<RecentCaptures captures={recent} />

		<div class="tools">
			{#each tools(sources) as tool (tool.id)}
				<Boundary what="the {tool.name} connector"><ToolCard {tool} {origins} {review} /></Boundary>
			{/each}
		</div>
	{/snippet}
</Deferred>

<aside class="help">
	<h2>Adding or fixing a connector</h2>
	<p>
		Connectors are YAML files bundled with GitPrompt. To patch one without a release, copy it to
		<code>~/.gitprompt/sources/</code>, edit it, then use <strong>Re-map stored events</strong> on
		its card. Each card's <strong>Spec in use</strong> shows which file it runs on. A file that can't
		be read is listed at the top of this page; the other connectors keep working, and a broken copy of
		a bundled spec falls back to the bundled one.
	</p>
</aside>

<style>
	.problems {
		margin-bottom: var(--space-4);
	}

	.tools {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(100%, 32rem), 1fr));
		gap: var(--space-4);
		align-items: start;
	}

	.help {
		max-width: var(--reading-max);
		margin-top: var(--space-8);
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}

	.help h2 {
		margin-bottom: var(--space-1);
		color: var(--color-text);
		font-size: var(--text-md);
	}
</style>
