<script lang="ts">
	import { ArrowLeft } from '@lucide/svelte';
	import { resolve } from '$app/paths';
	import ProjectStory from '#lib/stories/ProjectStory.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<svelte:head><title>{data.story.name} · GitPrompt</title></svelte:head>

<a class="back" href={resolve('/projects')}><ArrowLeft size={14} aria-hidden="true" />Projects</a>
<ProjectStory story={data.story} />

<style>
	.back {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
		margin-bottom: var(--space-3);
		font-size: var(--text-sm);
	}
</style>
