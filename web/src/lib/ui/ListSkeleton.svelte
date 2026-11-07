<script lang="ts">
	import Skeleton from '#lib/ui/Skeleton.svelte';

	/**
	 * A list while it loads. `cards`: separate outlined cards (sessions, projects, connectors);
	 * `rows`: dense rows in one surface (prompts, search results).
	 */
	let {
		rows = 6,
		variant = 'cards',
		height
	}: {
		rows?: number;
		variant?: 'cards' | 'rows';
		/** Height of each card's body, for cards taller than two lines (charts, steps). */
		height?: string;
	} = $props();

	const widths = ['82%', '64%', '73%', '58%', '88%', '69%'];
</script>

<div class={['list', variant]} aria-hidden="true">
	{#each { length: rows }, i (i)}
		<div class="item">
			<div class="line">
				<Skeleton width={widths[i % widths.length]} height="0.9rem" />
				<Skeleton width="3.5rem" height="0.7rem" />
			</div>
			<Skeleton width="40%" height="0.7rem" />
			{#if height}<Skeleton shape="block" {height} />{/if}
		</div>
	{/each}
</div>

<style>
	.list {
		display: grid;
		gap: var(--space-3);
	}

	.item {
		display: grid;
		gap: var(--space-2);
	}

	.line {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: var(--space-4);
	}

	.cards .item {
		padding: var(--space-4) var(--space-5);
		border-radius: var(--radius-lg);
		background: var(--color-surface);
		box-shadow: inset 0 0 0 1px var(--color-border);
	}

	.rows {
		gap: 0;
		padding: var(--space-1);
		border-radius: var(--radius-lg);
		background: var(--color-surface);
		box-shadow: inset 0 0 0 1px var(--color-border);
	}

	.rows .item {
		padding: var(--space-3);
	}

	.rows .item + .item {
		border-top: var(--border-hairline) solid var(--color-border);
	}
</style>
