<script lang="ts">
	import type { Doorway } from '#lib/insights/filters.ts';
	import type { Overview } from '#lib/insights/overview.ts';
	import { slotNames, slotWindow, type Slot } from '#lib/insights/series.ts';
	import { money } from '#lib/shared/format.ts';
	import ColumnChart, { type Column } from '#lib/ui/charts/ColumnChart.svelte';
	import { countPanel } from '#lib/insights/counting.ts';
	import type { Availability } from '#lib/sources/capabilities.ts';
	import { missingNotice } from '#lib/sources/missing.ts';
	import Panel from '#lib/ui/Panel.svelte';
	import { READY, type PanelState } from '#lib/ui/panelState.ts';
	import CapabilityNote from './CapabilityNote.svelte';

	/**
	 * What each day (or hour, over 24 hours) cost. The prompts per day and the streaks are the week
	 * strip on the first screen (overview/WeekStrip.svelte), so they are not drawn twice.
	 */
	let {
		data,
		slots,
		link,
		availability
	}: { data: Overview; slots: Slot[]; link: Doorway; availability: Availability } = $props();

	const now = $derived(new Date(data.now));
	const hourly = $derived(data.hourly);
	const columns = $derived<Column[]>(
		slots.map((s) => ({
			key: s.key,
			...slotNames(s, hourly, now, slots.length),
			value: s.bucket?.cost_usd ?? 0
		}))
	);
	const slotOf = (c: Column) => slots.find((s) => s.key === c.key)!;
	const open = (c: Column) => link(slotWindow(slotOf(c), hourly));
	const view = $derived<PanelState>(
		data.activity.totals.cost_usd > 0
			? READY
			: { kind: 'unsupported', ...missingNotice('costs', availability) }
	);
</script>

<Panel
	{@attach countPanel('activity')}
	state={view}
	title={hourly ? 'What does each hour cost?' : 'What does each day cost?'}
>
	<ColumnChart
		label={hourly ? 'Spend per hour' : 'Spend per day'}
		data={columns}
		format={money}
		axisFormat={money}
		color="var(--chart-2)"
		hrefFor={open}
	/>
	<CapabilityNote {availability} />
</Panel>
