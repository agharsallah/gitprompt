<script lang="ts">
	import { ArrowLeft } from '@lucide/svelte';
	import { refreshAll } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { liveUpdates } from '#lib/api/live.svelte.ts';
	import ExportButton from '#lib/export/ExportButton.svelte';
	import SessionStory from '#lib/stories/SessionStory.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	$effect(() => liveUpdates(refreshAll, 3000));
</script>

<svelte:head><title>Session story · GitPrompt</title></svelte:head>

<a class="back" href={resolve('/sessions')}><ArrowLeft size={14} aria-hidden="true" />Sessions</a>
<SessionStory story={data.story}>
	{#snippet actions()}
		<ExportButton target={{ kind: 'session', id: data.story.session }} />
	{/snippet}
</SessionStory>

<style>
	.back {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
		margin-bottom: var(--space-3);
		font-size: var(--text-sm);
	}
</style>
