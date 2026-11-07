<script lang="ts" module>
	/** Series names with a key that mirrors the mark: a block for bars, a stroke for lines, a tick for lanes. */
	export type LegendItem = {
		label: string;
		color: string;
		shape?: 'block' | 'line' | 'tick' | 'dot';
		value?: string;
	};
</script>

<script lang="ts">
	let { items, label = 'Legend' }: { items: LegendItem[]; label?: string } = $props();
</script>

<ul class="legend" aria-label={label}>
	{#each items as item (item.label)}
		<li>
			<span class="key {item.shape ?? 'block'}" style:--key={item.color} aria-hidden="true"></span>
			<span>{item.label}</span>
			{#if item.value}<span class="value num">{item.value}</span>{/if}
		</li>
	{/each}
</ul>

<style>
	.legend {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1) var(--space-4);
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
	}

	li {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1_5);
	}

	.key {
		flex: none;
		background: var(--key);
	}

	.block {
		width: 10px;
		height: 10px;
		border-radius: 3px;
	}

	.line {
		width: 14px;
		height: 2px;
		border-radius: 1px;
	}

	.tick {
		width: 3px;
		height: 12px;
		border-radius: 2px;
	}

	.dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
	}

	.value {
		color: var(--color-text);
		font-weight: var(--weight-medium);
	}
</style>
