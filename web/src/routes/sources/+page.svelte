<script lang="ts">
	import { refreshAll } from '$app/navigation';
	import { liveUpdates } from '#lib/api/live.svelte.ts';
	import SourceCard from '#lib/sources/SourceCard.svelte';
	import PageHeader from '#lib/ui/PageHeader.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	$effect(() => liveUpdates(refreshAll, 2000));
</script>

<svelte:head><title>Sources · GitPrompt</title></svelte:head>

<PageHeader
	title="Sources"
	description="The tools GitPrompt captures from. It only uses each tool's documented extension points, and you review the exact change to its settings before anything is written."
/>

<div class="sources">
	{#each data.sources as source (source.id)}
		<SourceCard {source} />
	{/each}
</div>

<aside class="help">
	<h2>Adding or fixing a source</h2>
	<p>
		Sources are YAML files bundled with GitPrompt. To patch one without a release, copy it to
		<code>~/.gitprompt/sources/</code>, edit it, then use <strong>Re-map stored events</strong> on its
		card.
	</p>
</aside>

<style>
	.sources {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(100%, 26rem), 1fr));
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
