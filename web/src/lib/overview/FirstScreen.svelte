<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { StatsQuery } from '#lib/api/query.ts';
	import type { Attention, Source } from '#lib/api/types.ts';
	import NeedsYou from '#lib/overview/NeedsYou.svelte';
	import SetupSteps from '#lib/sources/SetupSteps.svelte';

	/**
	 * The top of the Overview, right under its header: what needs you (NeedsYou.svelte), and the
	 * setup steps while no tool is connected (alone on a first run: nothing was ever captured).
	 * The rest of the page (filters, key figures, panels) comes after, as `children`, so the lists
	 * are always first.
	 */
	let {
		sources,
		attention,
		calm,
		query,
		onchanged,
		now,
		children
	}: {
		sources: Source[];
		attention?: Attention;
		calm?: string;
		query?: StatsQuery;
		onchanged?: () => void;
		now?: number;
		children?: Snippet;
	} = $props();

	/** Something was ever captured: there is something to pick up or to look at. */
	const ever = $derived(sources.some((s) => s.total > 0));
	/** No tool sends anything any more (or never did): the setup steps say what to do. */
	const setup = $derived(!ever || !sources.some((s) => s.connected));
</script>

{#if setup}
	<SetupSteps {sources} />
{/if}
{#if ever}
	<NeedsYou {attention} {calm} {query} {onchanged} {now} />
{/if}
{@render children?.()}
