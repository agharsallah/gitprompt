<script lang="ts">
	import { goto, refreshAll } from '$app/navigation';
	import { page } from '$app/state';
	import { pageHref } from '#lib/shared/links.ts';
	import { liveUpdates } from '#lib/api/live.svelte.ts';
	import type { Capability, PromptQuery } from '#lib/api/types.ts';
	import Deferred from '#lib/async/Deferred.svelte';
	import { Resource } from '#lib/async/resource.svelte.ts';
	import Boundary from '#lib/errors/Boundary.svelte';
	import ExportButton from '#lib/export/ExportButton.svelte';
	import { clickThrough } from '#lib/insights/counting.ts';
	import FilterBar from '#lib/insights/FilterBar.svelte';
	import { promptsHref } from '#lib/insights/filters.ts';
	import type { Overview } from '#lib/insights/overview.ts';
	import { rangePhrase } from '#lib/insights/range.ts';
	import { chartStart, timeSlots } from '#lib/insights/series.ts';
	// The first screen is part of the page; the panels below it sit in tabs (overview/OverviewTabs).
	import KeyFigures from '#lib/insights/panels/KeyFigures.svelte';
	import { overviewTabHref, resolveTab, type OverviewTab } from '#lib/insights/tabs.ts';
	import { topProject } from '#lib/insights/keyFigures.ts';
	import { calmLine } from '#lib/overview/calm.ts';
	import FirstScreen from '#lib/overview/FirstScreen.svelte';
	import LeaksPanel from '#lib/overview/LeaksPanel.svelte';
	import NeedsYou from '#lib/overview/NeedsYou.svelte';
	import OverviewTabs from '#lib/overview/OverviewTabs.svelte';
	import WeekStrip from '#lib/overview/WeekStrip.svelte';
	import { keepOverviewTab, savedOverviewTab } from '#lib/settings/views.ts';
	import { track } from '#lib/shared/usage.ts';
	import { availability } from '#lib/sources/capabilities.ts';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import { PURPOSES } from '#lib/layout/nav.ts';
	import PanelSkeleton from '#lib/ui/PanelSkeleton.svelte';
	import Skeleton from '#lib/ui/Skeleton.svelte';
	import StoryHeader from '#lib/ui/StoryHeader.svelte';
	import { overviewHeadline, totalsLine } from '#lib/stories/headline.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	// Refetch once capture goes quiet, not on every event.
	$effect(() => liveUpdates(refreshAll, 3000));

	// The tab below the first screen: the URL's, else the one used last, else Activity. The tab
	// shown is the one kept, so a link that names a tab is remembered as well as a click.
	const tab = $derived(resolveTab(page.url.searchParams.get('tab'), savedOverviewTab()));
	$effect(() => keepOverviewTab(tab));

	/**
	 * A new history entry with the tab in the URL (Back returns to the tab before); scroll and focus
	 * stay. The page's load reads only its filters, so the numbers are not fetched again.
	 */
	function selectTab(id: OverviewTab) {
		track(`tab.overview.${id}`);
		void goto(overviewTabHref(page.url.search, id), { replace: false, reset: false });
	}

	const content = new Resource(() => data.content);
	const overview = $derived(content.value?.overview);
	const totals = $derived(overview?.keyFigures.totals);
	const summary = $derived(totals?.prompts ? totalsLine(totals) : 'Your prompting, at a glance.');
	const headline = $derived(
		overview && totals?.prompts
			? overviewHeadline({
					stats: overview.keyFigures,
					range: overview.filters.range.id,
					top: topProject(overview.repos)
				})
			: null
	);

	const linkFor =
		(o: Overview) =>
		(extra: PromptQuery = {}) =>
			promptsHref(pageHref('/prompts'), o.filters, extra);
	const slotsFor = (o: Overview) =>
		timeSlots(
			o.activity.groups,
			chartStart(o.start, o.activity.totals.first_at, o.now),
			new Date(o.now),
			o.hourly
		);
</script>

<svelte:head><title>Overview · GitPrompt</title></svelte:head>

<StoryHeader title="Overview" purpose={PURPOSES['/']} {headline} totals={summary}>
	{#snippet actions()}
		{#if overview && totals?.prompts}
			<ExportButton target={{ kind: 'overview', query: overview.query }} />
		{/if}
	{/snippet}
</StoryHeader>

{#snippet panel()}<PanelSkeleton />{/snippet}

<Deferred resource={content} label="Loading the overview" what="the overview">
	{#snippet pending()}
		<div class="overview">
			<NeedsYou />
			<div class="filters">
				{#each ['7rem', '6rem', '8rem', '6rem'] as width, i (i)}
					<Skeleton {width} height="2rem" shape="block" />
				{/each}
			</div>
			<PanelSkeleton body="figures" rows={4} title={false} />
			<Skeleton width="100%" height="6.5rem" shape="block" />
			<div class="grid two">
				{@render panel()}
				{@render panel()}
			</div>
		</div>
	{/snippet}

	{#snippet children({ overview: o, sources, recap })}
		{@const t = o.activity.totals}
		{@const link = linkFor(o)}
		{@const slots = slotsFor(o)}
		{@const can = (c: Capability) => availability(sources, c)}
		{@const ever = sources.some((s) => s.total > 0)}
		<div class="overview" use:clickThrough>
			<FirstScreen
				{sources}
				attention={o.attention}
				{recap}
				calm={calmLine(o.filters.range, o.keyFigures.totals)}
				query={o.query}
				onchanged={() => refreshAll()}
			>
				<FilterBar filters={o.filters} origins={o.origins} options={o.options} />

				{#if !t.prompts}
					{#if ever}
						<EmptyState title="Nothing {rangePhrase(o.filters.range)} for these filters">
							Pick a longer range, more origins or fewer filters, or open the
							<a href={pageHref('/prompts')} title="Prompts">full list of prompts</a>.
						</EmptyState>
					{/if}
				{:else}
					<Boundary what="the key figures">
						<KeyFigures
							stats={o.keyFigures}
							repos={o.repos}
							range={o.filters.range}
							{slots}
							{link}
							can={{ cost: can('cost'), turnEnd: can('turn_end'), tools: can('tools') }}
						/>
					</Boundary>

					<Boundary what="the week strip"><WeekStrip data={o} /></Boundary>

					<Boundary what="the possible leaks">
						<LeaksPanel
							leaks={o.leaks}
							query={o.query}
							onchanged={() => refreshAll()}
							now={new Date(o.now)}
						/>
					</Boundary>

					<OverviewTabs
						selected={tab}
						onselect={selectTab}
						data={o}
						{link}
						{slots}
						{can}
						weekStart={data.weekStart}
					/>
				{/if}
			</FirstScreen>
		</div>
	{/snippet}
</Deferred>

<style>
	.overview {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		gap: var(--space-5);
	}

	.filters {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}

	.grid {
		display: grid;
		gap: var(--space-5);
		align-items: start;
	}

	.two {
		grid-template-columns: repeat(auto-fit, minmax(min(100%, 26rem), 1fr));
	}
</style>
