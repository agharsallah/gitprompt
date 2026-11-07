<script lang="ts">
	import type { Doorway } from '#lib/insights/filters.ts';
	import type { Overview } from '#lib/insights/overview.ts';
	import { slotWindow, streaks, type Slot } from '#lib/insights/series.ts';
	import { compact, dayKey, dayLabel, money, plural } from '#lib/shared/format.ts';
	import ColumnChart, { type Column } from '#lib/ui/charts/ColumnChart.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import Panel from '#lib/ui/Panel.svelte';

	/** How much you prompt, day by day (or hour by hour over 24 hours), and how steadily. */
	let { data, slots, link }: { data: Overview; slots: Slot[]; link: Doorway } = $props();

	const now = $derived(new Date(data.now));
	const hourly = $derived(data.hourly);
	const run = $derived(streaks(slots));
	const time = (d: Date) => d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
	const columns = (value: (s: Slot) => number): Column[] =>
		slots.map((s) => ({
			key: s.key,
			label: hourly
				? `${time(s.date)}, ${dayLabel(dayKey(s.date), now).toLowerCase()}`
				: dayLabel(s.key, now),
			tick: hourly
				? s.date.toLocaleTimeString([], { hour: '2-digit' })
				: s.date.toLocaleDateString(
						'en-GB',
						slots.length > 10 ? { day: 'numeric', month: 'short' } : { weekday: 'short' }
					),
			value: value(s)
		}));
	const slotOf = (c: Column) => slots.find((s) => s.key === c.key)!;
	const open = (c: Column) => link(slotWindow(slotOf(c), hourly));
	const cost = $derived(data.activity.totals.cost_usd > 0);
	const description = $derived(
		hourly
			? undefined
			: `Active on ${plural(run.active, 'day')} of ${run.days}; longest streak ${plural(run.longest, 'day')}${run.current ? `, current ${plural(run.current, 'day')}` : ''}.`
	);
</script>

<Panel
	title={hourly ? 'How much do I prompt each hour?' : 'How much do I prompt each day?'}
	{description}
>
	{#if data.activity.totals.prompts}
		<ColumnChart
			label={hourly ? 'Prompts per hour' : 'Prompts per day'}
			data={columns((s) => s.bucket?.prompts ?? 0)}
			format={(v) => plural(v, 'prompt')}
			axisFormat={compact}
			hrefFor={open}
		/>
		{#if cost}
			<h3 class="sub">{hourly ? 'Spend per hour' : 'Spend per day'}</h3>
			<ColumnChart
				label={hourly ? 'Spend per hour' : 'Spend per day'}
				data={columns((s) => s.bucket?.cost_usd ?? 0)}
				height={80}
				format={money}
				axisFormat={money}
				color="var(--chart-2)"
				hrefFor={open}
			/>
		{/if}
	{:else}
		<EmptyState compact title="No prompts in this window"
			>Pick a longer range or other filters.</EmptyState
		>
	{/if}
</Panel>

<style>
	.sub {
		margin: var(--space-5) 0 var(--space-2);
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
		font-weight: var(--weight-medium);
		font-stretch: var(--stretch-normal);
		letter-spacing: var(--tracking-normal);
	}
</style>
