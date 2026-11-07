<script lang="ts">
	/**
	 * A trend in a stat tile: one 2px line, a 10% wash, and the latest point marked. Decorative
	 * (aria-hidden) unless `label` is given: the tile's text already states the value.
	 */
	let {
		values,
		width = 96,
		height = 28,
		color = 'var(--color-accent)',
		label
	}: { values: number[]; width?: number; height?: number; color?: string; label?: string } = $props();

	const pad = 3;
	const max = $derived(Math.max(...values, 0));
	const points = $derived(
		values.map((v, i) => [
			pad + (values.length > 1 ? (i / (values.length - 1)) * (width - pad * 2) : (width - pad * 2) / 2),
			height - pad - (max ? (v / max) * (height - pad * 2) : 0)
		])
	);
	const line = $derived(points.map(([x, y], i) => `${i ? 'L' : 'M'}${x.toFixed(1)},${y.toFixed(1)}`).join(''));
	const area = $derived(
		points.length ? `${line}L${points.at(-1)![0]},${height - pad}L${points[0][0]},${height - pad}Z` : ''
	);
	const last = $derived(points.at(-1));
</script>

{#if values.length}
	<svg
		class="spark"
		{width}
		{height}
		viewBox="0 0 {width} {height}"
		role={label ? 'img' : undefined}
		aria-label={label}
		aria-hidden={label ? undefined : 'true'}
		style:--spark={color}
	>
		<path class="area" d={area} />
		<path class="line" d={line} />
		{#if last}<circle cx={last[0]} cy={last[1]} r="3" />{/if}
	</svg>
{/if}

<style>
	.spark {
		overflow: visible;
	}

	.area {
		fill: var(--spark);
		fill-opacity: 0.1;
	}

	.line {
		fill: none;
		stroke: var(--spark);
		stroke-width: 2;
		stroke-linecap: round;
		stroke-linejoin: round;
	}

	circle {
		fill: var(--spark);
		stroke: var(--color-surface);
		stroke-width: 2;
	}
</style>
