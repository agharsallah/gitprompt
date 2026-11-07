<script lang="ts">
	import { Plug } from '@lucide/svelte';
	import type { StatsBucket } from '#lib/api/types.ts';
	import LayerStep from '#lib/sources/LayerStep.svelte';
	import { CAPABILITIES, CAPABILITY_IDS } from '#lib/sources/capabilities.ts';
	import type { Tool } from '#lib/sources/state.ts';

	/** One tool and its connectors as steps: the main layer first, then the optional ones. */
	let {
		tool,
		origins = {},
		review = null
	}: {
		tool: Tool;
		/** Prompts by origin, per connector id. */
		origins?: Record<string, StatsBucket[]>;
		/** The connector whose consent review opens on arrival. */
		review?: string | null;
	} = $props();

	const version = $derived(tool.layers[0].version);
	// What this tool can feed into the dashboard at all (any layer), and what no layer provides.
	const declared = $derived(new Set(tool.layers.flatMap((l) => l.capabilities)));
	const missing = $derived(CAPABILITY_IDS.filter((c) => !declared.has(c)));
</script>

<article class="tool" id="tool-{tool.id}" aria-labelledby="tool-{tool.id}-name">
	<header>
		<span class="icon" aria-hidden="true"><Plug size={18} /></span>
		<div>
			<h2 id="tool-{tool.id}-name">{tool.name}</h2>
			<p class="muted version">{version ? `Version ${version}` : 'Version unknown'}</p>
		</div>
	</header>
	{#if missing.length && declared.size}
		<p class="missing">
			<span class="label">Not available from this tool:</span>
			{missing.map((c) => CAPABILITIES[c].what).join(', ')}. Pages that need them say so instead of
			showing zeros.
		</p>
	{/if}
	<ol class="layers">
		{#each tool.layers as layer, i (layer.id)}
			<LayerStep
				source={layer}
				step={i + 1}
				optional={i > 0}
				origins={origins[layer.id]}
				autoReview={review === layer.id}
			/>
		{/each}
	</ol>
</article>

<style>
	.tool {
		display: grid;
		gap: var(--space-5);
		min-width: 0;
		padding: var(--space-5);
		border-radius: var(--radius-lg);
		background: var(--color-surface);
		box-shadow: inset 0 0 0 1px var(--color-border);
	}

	header {
		display: flex;
		align-items: center;
		gap: var(--space-3);
	}

	.icon {
		display: grid;
		place-items: center;
		width: 2.25rem;
		height: 2.25rem;
		border-radius: var(--radius-md);
		background: var(--color-accent-soft);
		color: var(--color-accent);
	}

	h2 {
		font-size: var(--text-lg);
	}

	.version {
		font-size: var(--text-xs);
	}

	.missing {
		max-width: var(--reading-max);
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}

	.missing .label {
		color: var(--color-text);
		font-weight: var(--weight-medium);
	}

	.layers {
		display: grid;
		gap: var(--space-5);
	}
</style>
