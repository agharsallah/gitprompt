<script lang="ts">
	import { refreshAll } from '$app/navigation';
	import { pageHref } from '#lib/shared/links.ts';
	import { liveUpdates } from '#lib/api/live.svelte.ts';
	import type { Capability, PromptQuery } from '#lib/api/types.ts';
	import Deferred from '#lib/async/Deferred.svelte';
	import Lazy from '#lib/async/Lazy.svelte';
	import { Resource } from '#lib/async/resource.svelte.ts';
	import Boundary from '#lib/errors/Boundary.svelte';
	import ExportButton from '#lib/export/ExportButton.svelte';
	import { clickThrough } from '#lib/insights/counting.ts';
	import FilterBar from '#lib/insights/FilterBar.svelte';
	import { promptsHref } from '#lib/insights/filters.ts';
	import type { Overview } from '#lib/insights/overview.ts';
	import { rangePhrase } from '#lib/insights/range.ts';
	import { chartStart, timeSlots } from '#lib/insights/series.ts';
	// The first screen is part of the page; the panels further down load as they come into view.
	import ActivityPanel from '#lib/insights/panels/ActivityPanel.svelte';
	import KeyFigures from '#lib/insights/panels/KeyFigures.svelte';
	import RhythmPanel from '#lib/insights/panels/RhythmPanel.svelte';
	import SessionsPanel from '#lib/insights/panels/SessionsPanel.svelte';
	import { topProject } from '#lib/insights/keyFigures.ts';
	import { calmLine } from '#lib/overview/calm.ts';
	import FirstScreen from '#lib/overview/FirstScreen.svelte';
	import NeedsYou from '#lib/overview/NeedsYou.svelte';
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
{#snippet rows()}<PanelSkeleton body="rows" rows={3} />{/snippet}

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
			<div class="grid two">
				{@render panel()}
				{@render panel()}
			</div>
		</div>
	{/snippet}

	{#snippet children({ overview: o, sources })}
		{@const t = o.activity.totals}
		{@const link = linkFor(o)}
		{@const slots = slotsFor(o)}
		{@const can = (c: Capability) => availability(sources, c)}
		{@const ever = sources.some((s) => s.total > 0)}
		<div class="overview" use:clickThrough>
			<FirstScreen
				{sources}
				attention={o.attention}
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

					<section aria-labelledby="activity">
						<h2 id="activity" class="section">Activity</h2>
						<div class="grid two">
							<Boundary what="the activity panel"
								><ActivityPanel data={o} {slots} {link} /></Boundary
							>
							<Boundary what="the rhythm panel">
								<RhythmPanel data={o} weekStart={data.weekStart} {link} />
							</Boundary>
						</div>
						<Boundary what="the sessions panel"><SessionsPanel data={o} /></Boundary>
					</section>

					<section aria-labelledby="types">
						<h2 id="types" class="section">Prompt types</h2>
						<div class="grid two">
							<Lazy
								load={() => import('#lib/insights/panels/OriginPanel.svelte')}
								props={{ data: o, link }}
								when="visible"
								label="Loading who wrote the prompts"
								what="the origins panel"
								pending={panel}
							/>
							<Lazy
								load={() => import('#lib/insights/panels/CommandsPanel.svelte')}
								props={{ data: o, link, availability: can('session_metrics') }}
								when="visible"
								label="Loading slash commands"
								what="the commands panel"
								pending={panel}
							/>
							<Lazy
								load={() => import('#lib/insights/panels/WritingPanel.svelte')}
								props={{ data: o, link }}
								when="visible"
								label="Loading prompt sizes"
								what="the writing panel"
								pending={panel}
							/>
							<Lazy
								load={() => import('#lib/insights/panels/ReusePanel.svelte')}
								props={{ data: o, link }}
								when="visible"
								label="Loading reused prompts"
								what="the reuse panel"
								pending={panel}
							/>
						</div>
					</section>

					<section aria-labelledby="cost">
						<h2 id="cost" class="section">Models and cost</h2>
						<div class="grid two">
							<Lazy
								load={() => import('#lib/insights/panels/ModelsPanel.svelte')}
								props={{ data: o, link, availability: can('model') }}
								when="visible"
								label="Loading models"
								what="the models panel"
								pending={panel}
							/>
							<Lazy
								load={() => import('#lib/insights/panels/SpendPanel.svelte')}
								props={{ data: o, link, availability: can('cost') }}
								when="visible"
								label="Loading spend"
								what="the spend panel"
								pending={panel}
							/>
						</div>
					</section>

					<section aria-labelledby="work">
						<h2 id="work" class="section">Performance and agentic work</h2>
						<div class="grid two">
							<Lazy
								load={() => import('#lib/insights/panels/PerformancePanel.svelte')}
								props={{ data: o, link }}
								when="visible"
								label="Loading performance"
								what="the performance panel"
								pending={panel}
							/>
							<Lazy
								load={() => import('#lib/insights/panels/AgentPanel.svelte')}
								props={{ data: o, link }}
								when="visible"
								label="Loading agent work"
								what="the agent panel"
								pending={panel}
							/>
							<Lazy
								load={() => import('#lib/insights/panels/DelegationPanel.svelte')}
								props={{ data: o, link }}
								when="visible"
								label="Loading delegation"
								what="the delegation panel"
								pending={panel}
							/>
							<Lazy
								load={() => import('#lib/insights/panels/OutcomesPanel.svelte')}
								props={{ data: o, link, availability: can('session_metrics') }}
								when="visible"
								label="Loading outcomes"
								what="the outcomes panel"
								pending={panel}
							/>
						</div>
					</section>

					<Lazy
						load={() => import('#lib/insights/panels/StarredPanel.svelte')}
						props={{ prompts: o.starred, href: link({ starred: 'true' }) }}
						when="visible"
						label="Loading starred prompts"
						what="the starred prompts"
						pending={rows}
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

	section {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		gap: var(--space-4);
	}

	.section {
		margin-top: var(--space-3);
		color: var(--color-text);
		font-size: var(--text-lg);
		font-weight: var(--weight-semibold);
		font-stretch: var(--stretch-heading);
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
