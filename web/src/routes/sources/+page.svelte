<script lang="ts">
	import { onMount } from 'svelte';
	import { refreshAll, replaceState } from '$app/navigation';
	import { page } from '$app/state';
	import { liveUpdates } from '#lib/api/live.svelte.ts';
	import { tools } from '#lib/sources/state.ts';
	import ToolCard from '#lib/sources/ToolCard.svelte';
	import PageHeader from '#lib/ui/PageHeader.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	$effect(() => liveUpdates(refreshAll, 2000));

	const list = $derived(tools(data.sources));
	// First run: /sources?connect=<id> opens that connector's consent review straight away. Read
	// once, then dropped from the URL, so a reload doesn't reopen it.
	const review = page.url.searchParams.get('connect');
	onMount(() => {
		if (!review) return;
		const url = new URL(page.url.href);
		url.searchParams.delete('connect');
		replaceState(url, page.state);
	});
</script>

<svelte:head><title>Connectors · GitPrompt</title></svelte:head>

<PageHeader
	title="Connectors"
	description="The tools GitPrompt captures from. It only uses each tool's documented extension points, and you review the exact change to its settings before anything is written."
/>

<div class="tools">
	{#each list as tool (tool.id)}
		<ToolCard {tool} origins={data.origins} {review} />
	{/each}
</div>

<aside class="help">
	<h2>Adding or fixing a connector</h2>
	<p>
		Connectors are YAML files bundled with GitPrompt. To patch one without a release, copy it to
		<code>~/.gitprompt/sources/</code>, edit it, then use <strong>Re-map stored events</strong> on its
		card.
	</p>
</aside>

<style>
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
