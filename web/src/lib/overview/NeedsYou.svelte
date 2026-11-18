<script lang="ts">
	import type { StatsQuery } from '#lib/api/query.ts';
	import type { Attention } from '#lib/api/types.ts';
	import PickUp from '#lib/overview/PickUp.svelte';
	import WorthALook from '#lib/overview/WorthALook.svelte';
	import Callout from '#lib/ui/Callout.svelte';

	/**
	 * What needs you, first on the Overview: Pick up and Worth a look side by side (stacked when
	 * narrow), skeletons while the feed is on its way (`attention` undefined), and "Nothing needs
	 * you" with the period in one line when the daemon says so.
	 */
	let {
		attention,
		calm = '',
		query = {},
		onchanged,
		now
	}: {
		attention?: Attention;
		/** The period in one line, for the calm state (calm.ts). */
		calm?: string;
		query?: StatsQuery;
		onchanged?: () => void;
		/** A fixed "now" for Pick up; default: the shared clock. */
		now?: number;
	} = $props();
</script>

<div class="needs-you" data-first-screen>
	{#if !attention}
		<div class="lists">
			<PickUp />
			<WorthALook />
		</div>
	{:else if attention.nothing_needs_you}
		<Callout tone="success" title="Nothing needs you">{calm}</Callout>
	{:else}
		<div class="lists">
			<PickUp rows={attention.pick_up} {now} />
			<WorthALook items={attention.worth_a_look} {query} {onchanged} />
		</div>
	{/if}
</div>

<style>
	.lists {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(100%, 26rem), 1fr));
		gap: var(--space-5);
		align-items: start;
	}
</style>
