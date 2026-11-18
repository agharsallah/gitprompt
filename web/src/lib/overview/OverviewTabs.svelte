<script lang="ts">
	import type { Capability } from '#lib/api/types.ts';
	import type { Doorway } from '#lib/insights/filters.ts';
	import { STARRED_LIMIT, type Overview } from '#lib/insights/overview.ts';
	import type { Slot } from '#lib/insights/series.ts';
	import {
		OVERVIEW_TABS,
		panelAnchor,
		questionOf,
		tabEntry,
		tabNotice,
		type OverviewTab
	} from '#lib/insights/tabs.ts';
	import type { WeekStart } from '#lib/insights/week.ts';
	import TabPanel from '#lib/overview/TabPanel.svelte';
	import { anchorHref } from '#lib/shared/links.ts';
	import type { Availability } from '#lib/sources/capabilities.ts';
	import { takeaway } from '#lib/stories/takeaways.ts';
	import PanelState from '#lib/ui/PanelState.svelte';
	import Tabs from '#lib/ui/Tabs.svelte';

	/**
	 * The Overview below its first screen: five tabs (lib/insights/tabs.ts), each opening on one
	 * sentence from its own numbers, then "In this tab" (a jump link per panel), then its panels.
	 * Only the open tab's panels are drawn. The page owns which tab is open (the URL, else the one
	 * remembered) and what picking one does (`onselect`).
	 */
	let {
		selected,
		onselect,
		data,
		link,
		slots,
		can,
		weekStart
	}: {
		selected: OverviewTab;
		onselect: (tab: OverviewTab) => void;
		data: Overview;
		link: Doorway;
		slots: Slot[];
		can: (capability: Capability) => Availability;
		weekStart: WeekStart;
	} = $props();

	const tabs = OVERVIEW_TABS.map((t) => ({ id: t.id, label: t.label, hint: t.hint }));
	const starredHref = $derived(link({ starred: 'true' }));
</script>

<section class="below" aria-label="More about these prompts">
	<Tabs {tabs} {selected} label="Overview" {onselect}>
		{#snippet panel(id)}
			{@const entry = tabEntry(id)}
			{@const notice = tabNotice(entry, can)}
			{@const said = takeaway(id, data, STARRED_LIMIT)}
			<div class="tab" data-tab={id}>
				{#if notice}
					<PanelState state={{ kind: 'unsupported', ...notice }} />
				{:else if said}
					<p class="takeaway">{said}</p>
				{/if}
				<nav class="jump" aria-label="In this tab">
					<span class="lead">In this tab:</span>
					{#each entry.panels as p (p.panel)}
						<a href={anchorHref(panelAnchor(p.panel))} title={questionOf(p, data.hourly)}
							>{questionOf(p, data.hourly)}</a
						>
					{/each}
				</nav>
				<div class={['grid', entry.panels.length > 1 && 'two']}>
					{#each entry.panels as p (p.panel)}
						<div class="slot" id={panelAnchor(p.panel)}>
							<TabPanel name={p.panel} {data} {link} {slots} {can} {weekStart} {starredHref} />
						</div>
					{/each}
				</div>
			</div>
		{/snippet}
	</Tabs>
</section>

<style>
	.tab {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		gap: var(--space-4);
	}

	.takeaway {
		max-width: var(--reading-max);
		color: var(--color-text);
		font-size: var(--text-md);
		font-weight: var(--weight-semibold);
	}

	.jump {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
		font-size: var(--text-sm);
	}

	.lead {
		color: var(--color-text-muted);
	}

	.jump a {
		padding: var(--space-0_5) var(--space-2_5);
		border-radius: var(--radius-full);
		background: var(--color-accent-soft);
		text-decoration: none;
	}

	.grid {
		display: grid;
		gap: var(--space-5);
		align-items: start;
	}

	.two {
		grid-template-columns: repeat(auto-fit, minmax(min(100%, 26rem), 1fr));
	}

	.slot {
		min-width: 0;
		scroll-margin-top: var(--space-6);
	}
</style>
