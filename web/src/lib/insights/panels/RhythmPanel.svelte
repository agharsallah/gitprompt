<script lang="ts">
	import type { Doorway } from '#lib/insights/filters.ts';
	import type { Overview } from '#lib/insights/overview.ts';
	import { weekGrid } from '#lib/insights/series.ts';
	import type { WeekStart } from '#lib/insights/week.ts';
	import { plural } from '#lib/shared/format.ts';
	import Heatmap from '#lib/ui/charts/Heatmap.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import Panel from '#lib/ui/Panel.svelte';

	/** When in the week you prompt: weekday rows (week_start order) by local hour. */
	let { data, weekStart, link }: { data: Overview; weekStart: WeekStart; link: Doorway } = $props();

	const grid = $derived(weekGrid(data.week.groups, weekStart));
	const HOURS = Array.from({ length: 24 }, (_, h) => String(h).padStart(2, '0'));
</script>

<Panel title="When in the week do I prompt?" description="By weekday and hour, in your time zone.">
	{#if data.week.totals.prompts}
		<Heatmap
			label="Prompts by weekday and hour"
			values={grid.values}
			rows={grid.rows.map((d) => d.short)}
			columns={HOURS}
			columnTick={(c, i) => (i % 6 === 0 ? `${c}:00` : '')}
			describe={(r, c, v) =>
				`${plural(v, 'prompt')} on ${grid.rows.find((d) => d.short === r)?.plural} at ${c}:00`}
			hrefFor={(r, c) =>
				grid.values[r][c] ? link({ weekday: String(grid.isoDays[r]), hour: String(c) }) : undefined}
		/>
	{:else}
		<EmptyState compact title="No prompts in this window" />
	{/if}
</Panel>
