<script lang="ts">
	import { Coins, GitCommitHorizontal, Hourglass, Redo2, ShieldAlert } from '@lucide/svelte';
	import { countPanel } from '#lib/insights/counting.ts';
	import type { Doorway } from '#lib/insights/filters.ts';
	import { keyFigures, type FigureId, type Reporters } from '#lib/insights/keyFigures.ts';
	import type { Range } from '#lib/insights/range.ts';
	import type { Stats } from '#lib/api/types.ts';
	import type { Slot } from '#lib/insights/series.ts';
	import { anchorHref, LEAKS_ANCHOR } from '#lib/shared/links.ts';
	import StatTile from '#lib/ui/StatTile.svelte';

	/** The five figures you can act on, each against the period before and a link to its prompts. */
	let {
		stats,
		repos,
		range,
		slots,
		link,
		can
	}: {
		/** /api/stats with `key_figures` and `previous`. */
		stats: Stats;
		/** Projects by spend, most first. */
		repos: Stats;
		range: Range;
		slots: Slot[];
		link: Doorway;
		can: Reporters;
	} = $props();

	const ICONS = {
		redone: Redo2,
		agent_time: Hourglass,
		spend: Coins,
		commits: GitCommitHorizontal,
		leaks: ShieldAlert
	} satisfies Record<FigureId, unknown>;

	const figures = $derived(
		keyFigures({
			stats,
			repos,
			range,
			slots,
			link,
			// The panel is on this page, under the week strip: the filters in the URL stay.
			leaksHref: anchorHref(LEAKS_ANCHOR),
			can
		})
	);
</script>

<div class="tiles" {@attach countPanel('key-figures')}>
	{#each figures as { id, ...figure } (id)}
		<StatTile {...figure} icon={ICONS[id]} />
	{/each}
</div>

<style>
	.tiles {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(100%, 12rem), 1fr));
		gap: var(--space-3);
	}
</style>
