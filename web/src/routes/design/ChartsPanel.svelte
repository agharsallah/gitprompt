<script lang="ts">
	import BarList from '#lib/ui/charts/BarList.svelte';
	import ColumnChart from '#lib/ui/charts/ColumnChart.svelte';
	import Heatmap from '#lib/ui/charts/Heatmap.svelte';
	import Meter from '#lib/ui/charts/Meter.svelte';
	import SegmentedBar from '#lib/ui/charts/SegmentedBar.svelte';
	import Sparkline from '#lib/ui/charts/Sparkline.svelte';
	import Panel from '#lib/ui/Panel.svelte';
	import StatTile from '#lib/ui/StatTile.svelte';

	/** Figures and every chart, drawn from small made-up samples. */

	const DAYS = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
	const HOURS = Array.from({ length: 24 }, (_, h) => String(h));

	const tokens = [
		{ key: 'i', label: 'Input', value: 12, color: 'var(--chart-1)' },
		{ key: 'o', label: 'Output', value: 8, color: 'var(--chart-2)' },
		{ key: 'w', label: 'Cache write', value: 20, color: 'var(--chart-3)' },
		{ key: 'r', label: 'Cache read', value: 60, color: 'var(--chart-4)' }
	];
	const tools = [
		{ key: 'b', label: 'Bash', value: 42, color: 'var(--chart-3)' },
		{ key: 'r', label: 'Read', value: 30, color: 'var(--chart-1)' },
		{ key: 'e', label: 'Edit', value: 12, color: 'var(--chart-2)' }
	];
	const perDay = [3, 8, 5, 12, 9, 2, 6].map((value, i) => ({
		key: String(i + 1),
		label: `Day ${i + 1}`,
		tick: DAYS[i],
		value
	}));
	// A busy working week and a quiet weekend, with some texture.
	const heat = DAYS.map((_, r) =>
		HOURS.map((_, c) =>
			Math.max(0, Math.round(Math.sin((c - 6) / 4) * 6 + (r < 5 ? 4 : -2) + ((r * c) % 3)))
		)
	);
</script>

<Panel title="Figures and charts">
	<div class="tiles">
		<StatTile label="Spend" value="$12.40" hint="$0.08 per prompt" trend={[2, 4, 3, 6, 5, 8, 7]} />
		<StatTile label="Tokens" value="4.2M" hint="81% from cache"
			><Meter value={0.81} label="From cache" /></StatTile
		>
		<StatTile label="Typical turn" value="42s" hint="1 in 10 take over 3m 05s" />
	</div>
	<div class="grid two charts">
		<div class="stack">
			<SegmentedBar variant="tall" label="Tokens by kind" parts={tokens} />
			<BarList label="Tools" items={tools} />
			<Sparkline values={[1, 3, 2, 5, 4, 7, 6, 9]} width={160} height={36} label="Sample trend" />
		</div>
		<div class="stack">
			<ColumnChart label="Prompts per day" emphasis="7" data={perDay} />
			<Heatmap
				label="Sample heatmap"
				values={heat}
				rows={DAYS}
				columns={HOURS}
				columnTick={(c, i) => (i % 6 === 0 ? c : '')}
				describe={(r, c, v) => `${v} on ${r} at ${c}:00`}
			/>
		</div>
	</div>
</Panel>

<style>
	.tiles {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(100%, 12rem), 1fr));
		gap: var(--space-3);
	}

	.charts {
		margin-top: var(--space-5);
	}
</style>
