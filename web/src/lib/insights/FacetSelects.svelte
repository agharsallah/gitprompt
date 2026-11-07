<script lang="ts">
	import { Bot, Flag, FolderGit2, Plug, type LucideIcon } from '@lucide/svelte';
	import type { Stats } from '#lib/api/types.ts';
	import { FACETS, facetChoices, type Facet } from '#lib/insights/filters.ts';
	import Select from '#lib/ui/Select.svelte';

	/**
	 * The select filters of the Overview and the Prompts page, in one row: tool, project, model,
	 * outcome. Native selects inside the page's GET form, so the URL holds the choice. Each choice is
	 * counted with the other filters applied (`options`, from /api/stats).
	 */
	let {
		facets,
		values,
		options,
		onchange
	}: {
		facets: readonly Facet[];
		/** The chosen value of each facet ('' or missing: any). */
		values: Partial<Record<Facet, string | undefined>>;
		options: Partial<Record<Facet, Stats>>;
		onchange: () => void;
	} = $props();

	const ICONS: Record<Facet, LucideIcon> = {
		source: Plug,
		repo: FolderGit2,
		model: Bot,
		outcome: Flag
	};
</script>

<div class="selects">
	{#each facets as facet (facet)}
		<Select
			label={FACETS[facet].label}
			hideLabel
			name={facet}
			icon={ICONS[facet]}
			anyLabel={FACETS[facet].any}
			value={values[facet] ?? ''}
			options={facetChoices(facet, options[facet], values[facet])}
			{onchange}
		/>
	{/each}
</div>

<style>
	.selects {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
		min-width: 0;
	}

	/* A long project path must not stretch its select across the row. */
	.selects > :global(.field) {
		max-width: 16rem;
	}

	.selects :global(select) {
		min-width: 0;
		text-overflow: ellipsis;
	}
</style>
