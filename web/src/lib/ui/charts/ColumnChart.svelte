<script lang="ts" module>
	export type Column = {
		key: string;
		/** Full name, for the tooltip and the table view ("Thursday 5 November"). */
		label: string;
		/** Short axis label ("Thu"); defaults to label. */
		tick?: string;
		value: number;
	};
</script>

<script lang="ts">
	import { goto } from '$app/navigation';
	import Tooltip from '#lib/ui/charts/Tooltip.svelte';
	import { columnPath, niceTicks } from '#lib/ui/charts/scale.ts';
	import { compact } from '#lib/shared/format.ts';

	/**
	 * Magnitude over ordered categories (days, buckets). One series, one hue; `emphasis` highlights
	 * one column and dims the rest. Hover or arrow keys show each column's value; a table view
	 * carries every value for assistive tech.
	 */
	let {
		data,
		label,
		height = 140,
		format = compact,
		axisFormat = format,
		color = 'var(--chart-1)',
		emphasis,
		ticks: fixedTicks,
		onselect,
		hrefFor
	}: {
		data: Column[];
		label: string;
		/** Plot height in px; the axis band is added below it. */
		height?: number;
		format?: (value: number) => string;
		axisFormat?: (value: number) => string;
		color?: string;
		emphasis?: string;
		/** Axis ticks to use instead of 1-2-5 steps (e.g. durationTicks for time). */
		ticks?: number[];
		onselect?: (column: Column) => void;
		/** Makes each column a doorway: click or Enter opens this link (the table view links too). */
		hrefFor?: (column: Column) => string | undefined;
	} = $props();

	function select(column: Column) {
		if (onselect) return onselect(column);
		const href = hrefFor?.(column);
		if (href) goto(href);
	}
	const clickable = $derived(!!(onselect || hrefFor));

	const AXIS_W = 40;
	const AXIS_H = 22;
	const TOP = 8;

	let width = $state(0);
	let active = $state<number | null>(null);

	const ticks = $derived(fixedTicks ?? niceTicks(Math.max(...data.map((d) => d.value), 0)));
	const top = $derived(ticks.at(-1) || 1);
	const plotW = $derived(Math.max(width - AXIS_W, 0));
	const band = $derived(data.length ? plotW / data.length : 0);
	const barW = $derived(Math.max(2, Math.min(24, band * 0.68)));
	const y = (v: number) => TOP + height - (v / top) * height;
	const cx = (i: number) => AXIS_W + band * i + band / 2;
	// Thin out x labels so they never collide (about 44px per label).
	const every = $derived(Math.max(1, Math.ceil((data.length * 44) / Math.max(plotW, 1))));

	function key(event: KeyboardEvent) {
		if (!data.length) return;
		const last = data.length - 1;
		const next = {
			ArrowRight: Math.min(last, (active ?? -1) + 1),
			ArrowLeft: Math.max(0, (active ?? last + 1) - 1),
			Home: 0,
			End: last
		}[event.key];
		if (next !== undefined) {
			event.preventDefault();
			active = next;
		} else if (event.key === 'Enter' && active !== null) select(data[active]);
		else if (event.key === 'Escape') active = null;
	}
</script>

<figure class="column-chart" bind:clientWidth={width}>
	{#if width}
		<!-- One tab stop; arrow keys move between columns (the tooltip shows the value). The keyboard
		     readout is an enhancement: every value is also in the table view below. -->
		<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
		<svg
			{width}
			height={TOP + height + AXIS_H}
			role="img"
			aria-label="{label}. Use arrow keys to read each value{clickable
				? ', Enter to open it'
				: ''}."
			tabindex="0"
			onkeydown={key}
			onblur={() => (active = null)}
			onpointerleave={() => (active = null)}
		>
			{#each ticks as t (t)}
				<line class="grid" x1={AXIS_W} x2={width} y1={y(t)} y2={y(t)} />
				<text class="axis" x={AXIS_W - 8} y={y(t)} dy="0.32em" text-anchor="end"
					>{axisFormat(t)}</text
				>
			{/each}
			{#each data as d, i (d.key)}
				<path
					class={['col', emphasis && d.key !== emphasis && 'dim', active === i && 'active']}
					d={columnPath(cx(i) - barW / 2, y(d.value), barW, TOP + height - y(d.value))}
					style:fill={color}
				/>
				{#if i % every === 0 || i === data.length - 1}
					<text class="axis" x={cx(i)} y={TOP + height + 15} text-anchor="middle"
						>{d.tick ?? d.label}</text
					>
				{/if}
				<!-- The hit target is the whole band, not the painted column. -->
				<rect
					class={['hit', clickable && 'clickable']}
					x={AXIS_W + band * i}
					y={TOP}
					width={band}
					{height}
					role="presentation"
					onpointerenter={() => (active = i)}
					onclick={() => select(d)}
				/>
			{/each}
			<line class="baseline" x1={AXIS_W} x2={width} y1={TOP + height} y2={TOP + height} />
		</svg>
		{#if active !== null && data[active]}
			<Tooltip x={cx(active)} y={y(data[active].value) - 2} {width}>
				<strong>{format(data[active].value)}</strong>
				{data[active].label}
			</Tooltip>
		{/if}
	{/if}
	<table class="sr-only">
		<caption>{label}</caption>
		<tbody>
			{#each data as d (d.key)}
				{@const href = hrefFor?.(d)}
				<tr>
					<th scope="row"
						>{#if href}<a {href}>{d.label}</a>{:else}{d.label}{/if}</th
					>
					<td>{format(d.value)}</td>
				</tr>
			{/each}
		</tbody>
	</table>
</figure>

<style>
	.column-chart {
		position: relative;
		min-width: 0;
		margin: 0;
	}

	svg {
		display: block;
		overflow: visible;
		border-radius: var(--radius-sm);
	}

	.grid {
		stroke: var(--chart-grid);
		stroke-width: 1;
	}

	.baseline {
		stroke: var(--chart-axis);
		stroke-width: 1;
	}

	.axis {
		fill: var(--color-text-muted);
		font-size: 11px;
		font-variant-numeric: tabular-nums;
	}

	.col {
		transition: opacity var(--duration-fast) var(--ease-out);
	}

	.col.dim {
		fill: var(--chart-dim) !important;
	}

	.col.active {
		filter: brightness(1.12);
	}

	.hit {
		fill: transparent;
	}

	.clickable {
		cursor: pointer;
	}
</style>
