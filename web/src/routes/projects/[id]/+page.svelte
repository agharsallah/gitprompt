<script lang="ts">
	import { ArrowLeft } from '@lucide/svelte';
	import { pageHref } from '#lib/shared/links.ts';
	import Deferred from '#lib/async/Deferred.svelte';
	import { Resource } from '#lib/async/resource.svelte.ts';
	import ExportButton from '#lib/export/ExportButton.svelte';
	import ProjectTabs from '#lib/projects/ProjectTabs.svelte';
	import ProjectHeader from '#lib/stories/ProjectHeader.svelte';
	import ProjectStory from '#lib/stories/ProjectStory.svelte';
	import StorySkeleton from '#lib/stories/StorySkeleton.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const story = new Resource(
		() => data.story,
		() => data.id
	);
</script>

<svelte:head><title>{story.value?.name ?? 'Project'} · GitPrompt</title></svelte:head>

<a class="back" href={pageHref('/projects')} title="Projects"
	><ArrowLeft size={14} aria-hidden="true" />Projects</a
>
<Deferred resource={story} label="Loading the project" what="this project" variant="page">
	{#snippet pending()}<StorySkeleton />{/snippet}
	{#snippet children(story)}
		<ProjectHeader {story}>
			{#snippet actions()}
				<ExportButton target={{ kind: 'project', id: story.id }} />
			{/snippet}
		</ProjectHeader>
		<ProjectTabs project={story.id}>
			{#snippet journey()}<ProjectStory {story} />{/snippet}
		</ProjectTabs>
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
