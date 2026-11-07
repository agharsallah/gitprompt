<script lang="ts">
	import { ArrowLeft } from '@lucide/svelte';
	import { refreshAll } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { liveUpdates } from '#lib/api/live.svelte.ts';
	import Deferred from '#lib/async/Deferred.svelte';
	import { Resource } from '#lib/async/resource.svelte.ts';
	import ExportButton from '#lib/export/ExportButton.svelte';
	import SessionStory from '#lib/stories/SessionStory.svelte';
	import StorySkeleton from '#lib/stories/StorySkeleton.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	$effect(() => liveUpdates(refreshAll, 3000));
	// Keyed by id: another session never shows this one's story while it loads.
	const story = new Resource(
		() => data.story,
		() => data.id
	);
</script>

<svelte:head><title>Session story · GitPrompt</title></svelte:head>

<a class="back" href={resolve('/sessions')}><ArrowLeft size={14} aria-hidden="true" />Sessions</a>
<Deferred resource={story} label="Loading the session" what="this session" variant="page">
	{#snippet pending()}<StorySkeleton />{/snippet}
	{#snippet children(story)}
		<SessionStory {story}>
			{#snippet actions()}
				<ExportButton target={{ kind: 'session', id: story.session }} />
			{/snippet}
		</SessionStory>
	{/snippet}
</Deferred>

<style>
	.back {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
		margin-bottom: var(--space-3);
		font-size: var(--text-sm);
	}
</style>
