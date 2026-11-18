<script lang="ts" generics="C extends StackedColumn">
	import type { Snippet } from 'svelte';
	import { goto } from '$app/navigation';
	import { compact } from '#lib/shared/format.ts';
	import { rovingStep, tabStop } from '#lib/ui/charts/roving.ts';
	import { columnPath, niceTicks } from '#lib/ui/charts/scale.ts';
	import { stackSegments, type Series, type StackedColumn } from '#lib/ui/charts/stacked.ts';
	import Tooltip from '#lib/ui/charts/Tooltip.svelte';

	/**
	 * Ordered columns (days, hours) stacked by a few series, each column a doorway. The columns are
	 * links in one tab stop: ← → move between them, ↵ opens one (roving tabindex, roving.ts). One
	 * column can be outlined (today) and one marked (it stands out); `highlight` keeps one series
	 * and dims the others. A table view carries every value for assistive tech.
	 */
	let {
		data,
		series,
		label,
		height = 70,
		format = compact,
		axisFormat = format,
		outline,
		mark,
		highlight = null,
		hrefFor,
		describe = (c) => `${c.label}: ${format(c.value)}`,
		onopen,
		readout
	}: {
		data: C[];
		series: Series[];
		label: string;
		/** Plot height in px; the axis band is added below it. */
		height?: number;
		format?: (value: number) => string;
		axisFormat?: (value: number) => string;
		/** The key of the column drawn with an outline. */
		outline?: string;
		/** The key of the column with a mark above it. */
		mark?: string;
		/** The series kept in color; the others are dimmed. */
		highlight?: string | null;
		hrefFor: (column: C) => string;
		/** The accessible name of a column's link. */
		describe?: (column: C) => string;
		/** Called as a column is opened (click or ↵), before the page changes. */
		onopen?: (column: C) => void;
		/** What the hover readout shows for a column (default: its label and total). */
		readout?: Snippet<[C]>;
	} = $props();

	const AXIS_W = 40;
	const AXIS_H = 22;
	const TOP = 10;

	let width = $state(0);
	let active = $state<number | null>(null);
	let focused = $state<number | null>(null);
	const links = $state<HTMLAnchorElement[]>([]);

	const colors = $derived(new Map(series.map((s) => [s.key, s.color])));
	const names = $derived(new Map(series.map((s) => [s.key, s.label])));
	const ticks = $derived(niceTicks(Math.max(...data.map((d) => d.value), 0), 2));
	const top = $derived(ticks.at(-1) || 1);
	const plotW = $derived(Math.max(width - AXIS_W, 0));
	const band = $derived(data.length ? plotW / data.length : 0);
	const barW = $derived(Math.max(2, Math.min(24, band * 0.68)));
	const y = (v: number) => TOP + height - (v / top) * height;
	const cx = (i: number) => AXIS_W + band * i + band / 2;
	const every = $derived(Math.max(1, Math.ceil((data.length * 44) / Math.max(plotW, 1))));
	const stop = $derived(
		tabStop(
			focused,
			data.findIndex((d) => d.key === outline),
			data.length
		)
	);

	function open(column: C) {
		onopen?.(column);
		void goto(hrefFor(column));
	}

	function key(event: KeyboardEvent, index: number) {
		const step = rovingStep(event.key, index, data.length);
		if (step.kind === 'move') {
			event.preventDefault();
			focused = active = step.index;
			links[step.index]?.focus();
		} else if (step.kind === 'open') {
			event.preventDefault();
			open(data[step.index]);
		} else if (step.kind === 'leave') active = null;
	}

	const partsText = (c: StackedColumn) =>
		c.parts
			.filter((p) => p.value > 0)
			.map((p) => `${names.get(p.series) ?? p.series} ${format(p.value)}`)
			.join(', ');
</script>

<figure class="stacked" bind:clientWidth={width}>
	{#if width}
		<svg
			{width}
			height={TOP + height + AXIS_H}
			role="group"
			aria-label="{label}. Arrow keys move between columns, Enter opens one."
			onpointerleave={() => (active = null)}
		>
			{#each ticks as t (t)}
				<line class="grid" x1={AXIS_W} x2={width} y1={y(t)} y2={y(t)} />
				<text class="axis" x={AXIS_W - 8} y={y(t)} dy="0.32em" text-anchor="end"
					>{axisFormat(t)}</text
				>
			{/each}
			{#each data as d, i (d.key)}
				<a
					href={hrefFor(d)}
					tabindex={i === stop ? 0 : -1}
					aria-label={describe(d)}
					aria-current={d.key === outline ? 'date' : undefined}
					bind:this={links[i]}
					onkeydown={(e) => key(e, i)}
					onfocus={() => (focused = active = i)}
					onblur={() => (active = null)}
					onpointerenter={() => (active = i)}
					onclick={() => onopen?.(d)}
				>
					<!-- The hit target is the whole band, not the painted column. -->
					<rect class="hit" x={AXIS_W + band * i} y={TOP} width={band} {height} />
					{#if d.key === outline}
						<rect
							class="outline"
							x={AXIS_W + band * i + 1}
							y={TOP - 4}
							width={Math.max(band - 2, 0)}
							height={height + 4}
							rx="4"
						/>
					{/if}
					{#each stackSegments(d.parts) as s (s.series)}
						<path
							class={[
								'part',
								highlight && s.series !== highlight && 'dim',
								active === i && 'active'
							]}
							d={columnPath(cx(i) - barW / 2, y(s.to), barW, y(s.from) - y(s.to), s.top ? 4 : 0)}
							style:fill={colors.get(s.series)}
						/>
					{/each}
					{#if d.key === mark}
						<circle class="mark" cx={cx(i)} cy={y(d.value) - 6} r="3" />
					{/if}
				</a>
				{#if i % every === 0 || i === data.length - 1}
					<text class="axis" x={cx(i)} y={TOP + height + 15} text-anchor="middle"
						>{d.tick ?? d.label}</text
					>
				{/if}
			{/each}
			<line class="baseline" x1={AXIS_W} x2={width} y1={TOP + height} y2={TOP + height} />
		</svg>
		{#if active !== null && data[active]}
			<Tooltip x={cx(active)} y={y(data[active].value) - 4} {width}>
				{#if readout}{@render readout(data[active])}{:else}<strong
						>{format(data[active].value)}</strong
					>{data[active].label}{/if}
			</Tooltip>
		{/if}
	{/if}
	<table class="sr-only">
		<caption>{label}</caption>
		<tbody>
			{#each data as d (d.key)}
				<tr>
					<th scope="row"><a href={hrefFor(d)} tabindex="-1">{d.label}</a></th>
					<td>{format(d.value)}</td>
					<td>{partsText(d)}</td>
				</tr>
			{/each}
		</tbody>
	</table>
</figure>

<style>
	.stacked {
		position: relative;
		min-width: 0;
		margin: 0;
	}

	svg {
		display: block;
		overflow: visible;
	}

	a:focus-visible {
		outline: none;
	}

	a:focus-visible .hit {
		stroke: var(--color-accent);
		stroke-width: 2;
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

	.hit {
		fill: transparent;
		cursor: pointer;
	}

	.outline {
		fill: none;
		stroke: var(--color-border-control);
		stroke-width: 1;
		pointer-events: none;
	}

	.part {
		transition: opacity var(--duration-fast) var(--ease-out);
	}

	.part.dim {
		opacity: 0.25;
	}

	.part.active {
		filter: brightness(1.12);
	}

	.mark {
		fill: var(--color-text);
		pointer-events: none;
	}
</style>
