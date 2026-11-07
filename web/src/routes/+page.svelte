<script lang="ts">
	import { refreshAll } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { liveUpdates } from '#lib/api/live.svelte.ts';
	import type { PromptQuery } from '#lib/api/types.ts';
	import ExportButton from '#lib/export/ExportButton.svelte';
	import FilterBar from '#lib/insights/FilterBar.svelte';
	import { promptsHref } from '#lib/insights/filters.ts';
	import { rangePhrase } from '#lib/insights/range.ts';
	import { chartStart, timeSlots } from '#lib/insights/series.ts';
	import ActivityPanel from '#lib/insights/panels/ActivityPanel.svelte';
	import AgentPanel from '#lib/insights/panels/AgentPanel.svelte';
	import CommandsPanel from '#lib/insights/panels/CommandsPanel.svelte';
	import DelegationPanel from '#lib/insights/panels/DelegationPanel.svelte';
	import KeyFigures from '#lib/insights/panels/KeyFigures.svelte';
	import ModelsPanel from '#lib/insights/panels/ModelsPanel.svelte';
	import OriginPanel from '#lib/insights/panels/OriginPanel.svelte';
	import OutcomesPanel from '#lib/insights/panels/OutcomesPanel.svelte';
	import PerformancePanel from '#lib/insights/panels/PerformancePanel.svelte';
	import ReusePanel from '#lib/insights/panels/ReusePanel.svelte';
	import RhythmPanel from '#lib/insights/panels/RhythmPanel.svelte';
	import SessionsPanel from '#lib/insights/panels/SessionsPanel.svelte';
	import SpendPanel from '#lib/insights/panels/SpendPanel.svelte';
	import StarredPanel from '#lib/insights/panels/StarredPanel.svelte';
	import WritingPanel from '#lib/insights/panels/WritingPanel.svelte';
	import { plural } from '#lib/shared/format.ts';
	import SetupSteps from '#lib/sources/SetupSteps.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import PageHeader from '#lib/ui/PageHeader.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	// Refetch once capture goes quiet, not on every event.
	$effect(() => liveUpdates(refreshAll, 3000));

	const o = $derived(data.overview);
	const t = $derived(o.activity.totals);
	const link = $derived((extra: PromptQuery = {}) =>
		promptsHref(resolve('/prompts'), o.query, extra)
	);
	const slots = $derived(
		timeSlots(o.activity.groups, chartStart(o.start, t.first_at, o.now), new Date(o.now), o.hourly)
	);
	const telemetry = $derived(!!data.sources.find((s) => s.id === 'claude-code-otel')?.connected);
	const hooked = $derived(data.sources.some((s) => s.connected));
	const ever = $derived(data.sources.some((s) => s.total > 0));
	const summary = $derived(
		`${plural(t.prompts, 'prompt')} in ${plural(t.sessions, 'session')} ${rangePhrase(o.filters.range)}.`
	);
</script>

<svelte:head><title>Overview · GitPrompt</title></svelte:head>

<PageHeader title="Overview" description={t.prompts ? summary : 'Your prompting, at a glance.'}>
	{#snippet actions()}
		{#if t.prompts}<ExportButton target={{ kind: 'overview', query: o.query }} />{/if}
	{/snippet}
</PageHeader>

<div class="overview">
	{#if !hooked || !ever}
		<SetupSteps sources={data.sources} />
	{/if}

	<FilterBar
		filters={o.filters}
		fallback={data.defaultRange}
		origins={o.origins}
		options={o.options}
	/>

	{#if !t.prompts}
		{#if ever}
			<EmptyState title="Nothing {rangePhrase(o.filters.range)} for these filters">
				Pick a longer range, more origins or fewer filters, or open the
				<a href={resolve('/prompts')}>full list of prompts</a>.
			</EmptyState>
		{/if}
	{:else}
		<KeyFigures data={o} {slots} {link} />

		<section aria-labelledby="activity">
			<h2 id="activity" class="section">Activity</h2>
			<div class="grid two">
				<ActivityPanel data={o} {slots} {link} />
				<RhythmPanel data={o} weekStart={data.weekStart} {link} />
			</div>
			<SessionsPanel data={o} />
		</section>

		<section aria-labelledby="types">
			<h2 id="types" class="section">Prompt types</h2>
			<div class="grid two">
				<OriginPanel data={o} {link} />
				<CommandsPanel data={o} {link} connected={telemetry} />
				<WritingPanel data={o} {link} />
				<ReusePanel data={o} {link} />
			</div>
		</section>

		<section aria-labelledby="cost">
			<h2 id="cost" class="section">Models and cost</h2>
			<div class="grid two">
				<ModelsPanel data={o} {link} connected={telemetry} />
				<SpendPanel data={o} {link} connected={telemetry} />
			</div>
		</section>

		<section aria-labelledby="work">
			<h2 id="work" class="section">Performance and agentic work</h2>
			<div class="grid two">
				<PerformancePanel data={o} {link} />
				<AgentPanel data={o} {link} />
				<DelegationPanel data={o} {link} />
				<OutcomesPanel data={o} {link} connected={telemetry} />
			</div>
		</section>

		<StarredPanel prompts={o.starred} href={link({ starred: 'true' })} />
	{/if}
</div>

<style>
	.overview {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		gap: var(--space-5);
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
