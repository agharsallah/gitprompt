<script lang="ts">
	import type { StatsQuery } from '#lib/api/query.ts';
	import type { WorthItem } from '#lib/api/types.ts';
	import WorthRow from '#lib/overview/WorthRow.svelte';
	import Panel from '#lib/ui/Panel.svelte';
	import { readyOr, type PanelState } from '#lib/ui/panelState.ts';

	/**
	 * Up to three things worth a look in the range, in the daemon's order (leaks, redone turns,
	 * repeated prompts; never re-ranked here), each one sentence and one button.
	 */
	let {
		items,
		query = {},
		onchanged
	}: {
		/** The daemon's items; undefined while they are on their way (a skeleton). */
		items?: readonly WorthItem[];
		/** The Overview's filters: what a row acts on was found under them. */
		query?: StatsQuery;
		/** Something a row did changed the stored data (values hidden). */
		onchanged?: () => void;
	} = $props();

	const state: PanelState = $derived(
		items
			? readyOr(items.length, {
					kind: 'empty',
					title: 'Nothing stands out',
					message: 'No possible leak, redone turn or repeated prompt in this range.'
				})
			: { kind: 'loading', label: 'Loading what is worth a look', body: 'rows', rows: 3 }
	);
</script>

<Panel title="Worth a look" pad="sm" {state}>
	<ul class="items">
		{#each items ?? [] as item, i (i)}
			<WorthRow {item} {query} {onchanged} />
		{/each}
	</ul>
</Panel>

<style>
	.items {
		display: grid;
		gap: var(--space-2);
		margin: 0;
		padding: 0;
		list-style: none;
	}
</style>
