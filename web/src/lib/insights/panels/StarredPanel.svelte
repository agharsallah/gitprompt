<script lang="ts">
	import type { Prompt } from '#lib/api/types.ts';
	import { typedText } from '#lib/prompts/origin.ts';
	import { excerpt } from '#lib/shared/format.ts';
	import { promptHref } from '#lib/shared/links.ts';
	import { countPanel } from '#lib/insights/counting.ts';
	import Panel from '#lib/ui/Panel.svelte';
	import { readyOr } from '#lib/ui/panelState.ts';

	/** Prompts you marked as worth keeping, newest first, under the page's filters. */
	let { prompts, href }: { prompts: Prompt[]; href: string } = $props();

	const view = $derived(
		readyOr(prompts.length, {
			kind: 'empty',
			title: 'No starred prompts here',
			message: 'Star a prompt that worked well to find it again here.'
		})
	);
</script>

<Panel
	{@attach countPanel('starred')}
	title="Which prompts did I keep?"
	description="Prompts you starred."
	state={view}
>
	{#snippet actions()}{#if prompts.length}<a class="all" {href} title="Prompts › Starred"
				>All starred</a
			>{/if}{/snippet}
	<ul class="starred">
		{#each prompts as p (p.id)}
			<li>
				<a href={promptHref(p.id)} title="Prompt › {excerpt(typedText(p), 40)}"
					>{excerpt(typedText(p), 160)}</a
				>
			</li>
		{/each}
	</ul>
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
