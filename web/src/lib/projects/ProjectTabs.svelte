<script lang="ts">
	import type { Snippet } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import CommitTable from '#lib/projects/CommitTable.svelte';
	import { openTab, PROJECT_TABS, tabOf, type ProjectTab } from '#lib/projects/tabs.ts';
	import { track } from '#lib/shared/usage.ts';
	import Tabs from '#lib/ui/Tabs.svelte';

	/**
	 * A project's page below its header: the journey (`journey`, drawn by the page) and the commits,
	 * one tab apart. The tab is the URL's (`?tab=`), so it survives a reload and Back. `branch`: the
	 * project's current branch, whose prompts the Commits tab copies.
	 */
	let { project, branch, journey }: { project: string; branch?: string; journey: Snippet } =
		$props();

	const tab = $derived(tabOf(page.url));
	const tabs = PROJECT_TABS.map((t) => ({ id: t.id, label: t.label }));

	function select(id: ProjectTab) {
		if (id === 'commits') track('tab.project.commits');
		openTab(project, id, goto, page.url.searchParams.get('range') ?? undefined);
	}
</script>

<Tabs {tabs} selected={tab} label="Project" onselect={select}>
	{#snippet panel(id)}
		{#if id === 'commits'}<CommitTable {project} {branch} />{:else}{@render journey()}{/if}
	{/snippet}
</Tabs>
