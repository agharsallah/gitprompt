<script lang="ts">
	import { resolve } from '$app/paths';
	import type { Prompt } from '#lib/api/types.ts';
	import { typedText } from '#lib/prompts/origin.ts';
	import { excerpt } from '#lib/shared/format.ts';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import Panel from '#lib/ui/Panel.svelte';

	/** Prompts you marked as worth keeping, newest first, under the page's filters. */
	let { prompts, href }: { prompts: Prompt[]; href: string } = $props();
</script>

<Panel title="Which prompts did I keep?" description="Prompts you starred.">
	{#snippet actions()}{#if prompts.length}<a class="all" {href}>All starred</a>{/if}{/snippet}
	{#if prompts.length}
		<ul class="starred">
			{#each prompts as p (p.id)}
				<li><a href="{resolve('/prompts')}?id={p.id}">{excerpt(typedText(p), 160)}</a></li>
			{/each}
		</ul>
	{:else}
		<EmptyState compact title="No starred prompts here"
			>Star a prompt that worked well to find it again here.</EmptyState
		>
	{/if}
</Panel>

<style>
	.starred {
		display: grid;
		gap: var(--space-2);
	}

	.starred a {
		color: var(--color-text);
		text-decoration: none;
		overflow-wrap: anywhere;
	}

	.starred a:hover {
		color: var(--color-accent-text);
		text-decoration: underline;
	}

	.all {
		font-size: var(--text-sm);
	}
</style>
