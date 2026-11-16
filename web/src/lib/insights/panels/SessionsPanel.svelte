<script lang="ts">
	import type { Overview } from '#lib/insights/overview.ts';
	import { bands, lengthUnit, ratio, SESSION_LENGTHS } from '#lib/insights/series.ts';
	import { compact, plural } from '#lib/shared/format.ts';
	import { sessionsHref } from '#lib/shared/links.ts';
	import ColumnChart from '#lib/ui/charts/ColumnChart.svelte';
	import { countPanel } from '#lib/insights/counting.ts';
	import Panel from '#lib/ui/Panel.svelte';
	import { readyOr } from '#lib/ui/panelState.ts';

	/**
	 * How long sessions last, from their first prompt to the end of their last turn (all their
	 * prompts, whatever the filters: the filters choose which sessions count).
	 */
	let { data }: { data: Overview } = $props();

	const lengths = $derived(bands(data.sessionLengths.groups, SESSION_LENGTHS, lengthUnit));
	const t = $derived(data.activity.totals);
	const perSession = $derived(ratio(t.prompts, t.sessions));
	const view = $derived(
		readyOr(t.sessions, { kind: 'empty', title: 'No sessions in this window' })
	);
	const allSessions = $derived(sessionsHref({ project: data.filters.repo }));
</script>

<Panel
	{@attach countPanel('sessions')}
	state={view}
	title="How long do my sessions last?"
	description={t.sessions
		? `${plural(t.sessions, 'session')}, ${perSession !== null ? perSession.toFixed(1) : '0'} prompts each on average.`
		: undefined}
>
	{#snippet actions()}<a class="all" href={allSessions} title="Sessions">All sessions</a>{/snippet}
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
</Panel>

<style>
	.all {
		font-size: var(--text-sm);
	}
</style>
