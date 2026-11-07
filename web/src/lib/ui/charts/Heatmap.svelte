<script lang="ts">
	import Tooltip from '#lib/ui/charts/Tooltip.svelte';
	import { SEQ_STEPS, seqStep } from '#lib/ui/charts/scale.ts';

	/**
	 * A grid of magnitudes (rows × columns) in one sequential hue: more is stronger. One tab stop;
	 * arrow keys move the readout in two dimensions. A table view carries every value.
	 */
	let {
		values,
		rows,
		columns,
		label,
		columnTick = (_c: string, i: number) => (i % 3 === 0 ? _c : ''),
		describe
	}: {
		values: number[][];
		rows: string[];
		columns: string[];
		label: string;
		/** Axis text under each column (default: every third). */
		columnTick?: (column: string, index: number) => string;
		/** Tooltip and table text for one cell. */
		describe: (row: string, column: string, value: number) => string;
	} = $props();

	const max = $derived(Math.max(...values.flat(), 0));
	let width = $state(0);
	let active = $state<{ r: number; c: number; x: number; y: number } | null>(null);
	let cells: HTMLElement[] = $state([]);

	function point(r: number, c: number) {
		const el = cells[r * columns.length + c];
		if (el) active = { r, c, x: el.offsetLeft + el.offsetWidth / 2, y: el.offsetTop };
	}

	function key(event: KeyboardEvent) {
		const { r, c } = active ?? { r: 0, c: -1 };
		const next = {
			ArrowRight: [r, Math.min(columns.length - 1, c + 1)],
			ArrowLeft: [r, Math.max(0, c - 1)],
			ArrowDown: [Math.min(rows.length - 1, r + 1), Math.max(c, 0)],
			ArrowUp: [Math.max(0, r - 1), Math.max(c, 0)]
		}[event.key];
		if (next) {
			event.preventDefault();
			point(next[0], next[1]);
		} else if (event.key === 'Escape') active = null;
	}
</script>

<figure class="heatmap" bind:clientWidth={width}>
	<!-- One tab stop; arrow keys move the readout. An enhancement: the table view holds every value. -->
	<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
	<div
		class="grid"
		style:--cols={columns.length}
		role="img"
		aria-label="{label}. Use arrow keys to read each cell."
		tabindex="0"
		onkeydown={key}
		onblur={() => (active = null)}
		onpointerleave={() => (active = null)}
	>
		{#each rows as row, r (row)}
			<span class="row-label">{row}</span>
			{#each columns as column, c (column)}
				<!-- svelte-ignore a11y_no_static_element_interactions (pointer hover only; see the table view) -->
				<span
					bind:this={cells[r * columns.length + c]}
					class={['cell', active?.r === r && active?.c === c && 'active']}
					style:background="var(--chart-seq-{seqStep(values[r][c], max)})"
					onpointerenter={() => point(r, c)}
				></span>
			{/each}
		{/each}
		<span></span>
		{#each columns as column, c (column)}<span class="col-label">{columnTick(column, c)}</span
			>{/each}
	</div>
	{#if active}
		<Tooltip x={active.x} y={active.y - 2} {width}>
			<strong>{values[active.r][active.c]}</strong>
			{describe(rows[active.r], columns[active.c], values[active.r][active.c])}
		</Tooltip>
	{/if}
	<div class="scale" aria-hidden="true">
		Less
		{#each Array.from({ length: SEQ_STEPS + 1 }, (_, i) => i) as step (step)}
			<span class="cell" style:background="var(--chart-seq-{step})"></span>
		{/each}
		More
	</div>
	<table class="sr-only">
		<caption>{label}</caption>
		<tbody>
			{#each rows as row, r (row)}
				{#each columns as column, c (column)}
					{#if values[r][c]}<tr><td>{describe(row, column, values[r][c])}</td></tr>{/if}
				{/each}
			{/each}
		</tbody>
	</table>
</figure>

<style>
	.heatmap {
		position: relative;
		min-width: 0;
		margin: 0;
	}

	.grid {
		display: grid;
		grid-template-columns: max-content repeat(var(--cols), minmax(0, 1fr));
		gap: 2px;
		align-items: center;
		border-radius: var(--radius-sm);
	}

	.row-label {
		padding-right: var(--space-2);
		color: var(--color-text-muted);
		font-size: var(--text-xs);
	}

	.cell {
		height: 18px;
		min-width: 0;
		border-radius: 3px;
	}

	.grid .cell.active {
		outline: 2px solid var(--color-text);
		outline-offset: 1px;
	}

	.col-label {
		overflow: visible;
		color: var(--color-text-muted);
		font-size: 11px;
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
	}

	.scale {
		display: flex;
		align-items: center;
		justify-content: flex-end;
		gap: 3px;
		margin-top: var(--space-3);
		color: var(--color-text-muted);
		font-size: var(--text-xs);
	}

	.scale .cell {
		width: 12px;
		height: 12px;
	}

	.scale .cell:first-of-type {
		margin-left: var(--space-1);
	}

	.scale .cell:last-of-type {
		margin-right: var(--space-1);
	}
</style>
