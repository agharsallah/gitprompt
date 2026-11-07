<script lang="ts">
	import { Plug } from '@lucide/svelte';
	import { resolve } from '$app/paths';
	import type { Source } from '#lib/api/types.ts';
	import StateRow from '#lib/sources/StateRow.svelte';
	import { tools } from '#lib/sources/state.ts';
	import Button from '#lib/ui/Button.svelte';
	import Panel from '#lib/ui/Panel.svelte';

	/** Each tool and the state of its connectors; connecting happens on the Connectors page. */
	let { sources }: { sources: Source[] } = $props();

	const list = $derived(tools(sources));
</script>

<Panel
	title="Connectors"
	description="The tools GitPrompt captures from. Every change to a tool's settings is reviewed as a diff first."
>
	{#snippet actions()}
		<Button href={resolve('/sources')} icon={Plug} size="sm">Manage connectors</Button>
	{/snippet}
	<div class="tools">
		{#each list as tool (tool.id)}
			<section aria-labelledby="summary-{tool.id}">
				<h3 id="summary-{tool.id}">
					<a href="{resolve('/sources')}#tool-{tool.id}">{tool.name}</a>
				</h3>
				<ul>
					{#each tool.layers as layer (layer.id)}<li><StateRow source={layer} /></li>{/each}
				</ul>
			</section>
		{/each}
	</div>
</Panel>

<style>
	.tools {
		display: grid;
		gap: var(--space-4);
	}

	section {
		display: grid;
		gap: var(--space-2);
	}

	h3 {
		font-size: var(--text-md);
	}

	ul {
		display: grid;
		gap: var(--space-2);
	}
</style>
