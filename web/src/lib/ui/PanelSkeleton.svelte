<script lang="ts">
	import Skeleton from '#lib/ui/Skeleton.svelte';

	/**
	 * A Panel while its content loads: the same frame and padding, a title and description line,
	 * then a body shaped like what comes (a chart, rows of bars, a few figures or a paragraph), so
	 * the page does not jump when it arrives.
	 */
	let {
		body = 'chart',
		rows = 5,
		height = '11rem',
		title = true
	}: {
		body?: 'chart' | 'rows' | 'figures' | 'text';
		/** Rows for `rows`, figures for `figures`, lines for `text`. */
		rows?: number;
		/** Height of a `chart` body. */
		height?: string;
		title?: boolean;
	} = $props();

	// Bars of decreasing length read as "a ranked list" without pretending to be data.
	const widths = ['92%', '74%', '61%', '48%', '37%', '29%', '22%', '16%'];
</script>

<div class="panel" aria-hidden="true">
	{#if title}
		<div class="header">
			<Skeleton width="45%" height="1.05rem" />
			<Skeleton width="70%" height="0.75rem" />
		</div>
	{/if}
	{#if body === 'chart'}
		<Skeleton shape="block" {height} />
	{:else if body === 'rows'}
		<div class="rows">
			{#each { length: rows }, i (i)}
				<div class="row">
					<Skeleton width="28%" height="0.75rem" />
					<Skeleton width={widths[i % widths.length]} height="0.75rem" />
				</div>
			{/each}
		</div>
	{:else if body === 'figures'}
		<div class="figures">
			{#each { length: rows }, i (i)}
				<div class="figure">
					<Skeleton width="55%" height="0.7rem" />
					<Skeleton width="70%" height="1.6rem" />
				</div>
			{/each}
		</div>
	{:else}
		<Skeleton lines={rows} />
	{/if}
</div>

<style>
	.panel {
		display: grid;
		gap: var(--space-4);
		min-width: 0;
		padding: var(--space-5);
		border-radius: var(--radius-lg);
		background: var(--color-surface);
		box-shadow: inset 0 0 0 1px var(--color-border);
	}

	.header {
		display: grid;
		gap: var(--space-2);
	}

	.rows {
		display: grid;
		gap: var(--space-3);
	}

	.row {
		display: grid;
		grid-template-columns: minmax(6rem, 1fr) 2fr;
		gap: var(--space-3);
		align-items: center;
	}

	.figures {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(100%, 9rem), 1fr));
		gap: var(--space-5);
	}

	.figure {
		display: grid;
		gap: var(--space-2);
	}
</style>
