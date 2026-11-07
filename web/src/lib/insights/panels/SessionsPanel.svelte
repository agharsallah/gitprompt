<script lang="ts">
	import { resolve } from '$app/paths';
	import type { Overview } from '#lib/insights/overview.ts';
	import { bands, lengthUnit, ratio, SESSION_LENGTHS } from '#lib/insights/series.ts';
	import { compact, plural } from '#lib/shared/format.ts';
	import ColumnChart from '#lib/ui/charts/ColumnChart.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import Panel from '#lib/ui/Panel.svelte';

	/**
	 * How long sessions last, from their first prompt to the end of their last turn (all their
	 * prompts, whatever the filters: the filters choose which sessions count).
	 */
	let { data }: { data: Overview } = $props();

	const lengths = $derived(bands(data.sessionLengths.groups, SESSION_LENGTHS, lengthUnit));
	const t = $derived(data.activity.totals);
	const perSession = $derived(ratio(t.prompts, t.sessions));
	const sessionsHref = $derived(
		`${resolve('/sessions')}${data.filters.repo ? `?project=${encodeURIComponent(data.filters.repo)}` : ''}`
	);
</script>

<Panel
	title="How long do my sessions last?"
	description={t.sessions
		? `${plural(t.sessions, 'session')}, ${perSession !== null ? perSession.toFixed(1) : '0'} prompts each on average.`
		: undefined}
>
	{#snippet actions()}<a class="all" href={sessionsHref}>All sessions</a>{/snippet}
	{#if t.sessions}
		<ColumnChart
			label="Sessions by length"
			data={lengths.map((b) => ({
				key: b.key,
				label: `${b.label} (${plural(b.bucket?.prompts ?? 0, 'prompt')})`,
				tick: b.tick,
				value: b.bucket?.sessions ?? 0
			}))}
			height={110}
			format={(v) => plural(v, 'session')}
			axisFormat={compact}
		/>
	{:else}
		<EmptyState compact title="No sessions in this window" />
	{/if}
</Panel>

<style>
	.all {
		font-size: var(--text-sm);
	}
</style>
