<script lang="ts" module>
	import type { LucideIcon } from '@lucide/svelte';

	export type BarItem = {
		key: string;
		label: string;
		value: number;
		/** Text shown at the bar's end; defaults to format(value). */
		display?: string;
		/** Secondary text under the label. */
		hint?: string;
		href?: string;
		/** A swatch color when items are categories with their own identity. */
		color?: string;
		icon?: LucideIcon;
		mono?: boolean;
	};
</script>

<script lang="ts">
	import { integer } from '#lib/shared/format.ts';

	/**
	 * Ranked horizontal bars, largest first: compares parts more exactly than any pie. One hue for
	 * every bar (bar length already says "bigger"); per-item `color` only when items are categories
	 * with an identity used elsewhere. Values are printed at the bar end, so nothing hides behind hover.
	 */
	let {
		items,
		label,
		max = 8,
		otherLabel = 'Everything else',
		format = integer,
		color = 'var(--chart-1)'
	}: {
		items: BarItem[];
		label: string;
		/** Rows shown; the rest fold into one "Everything else" row. */
		max?: number;
		otherLabel?: string;
		format?: (value: number) => string;
		color?: string;
	} = $props();

	const rows = $derived.by(() => {
		if (items.length <= max) return items;
		const rest = items.slice(max - 1);
		const value = rest.reduce((a, i) => a + i.value, 0);
		return [
			...items.slice(0, max - 1),
			{
				key: '(other)',
				label: otherLabel,
				value,
				hint: `${rest.length} more`,
				color: 'var(--chart-other)'
			}
		];
	});
	const top = $derived(Math.max(...rows.map((r) => r.value), 0));
</script>

<ul class="bar-list" aria-label={label}>
	{#each rows as row (row.key)}
		{@const Icon = row.icon}
		<li>
			<svelte:element this={row.href ? 'a' : 'div'} class="row" href={row.href}>
				<span class="name">
					{#if Icon}<Icon
							size={14}
							aria-hidden="true"
							style="color: {row.color ?? color}"
						/>{:else if row.color}<span
							class="swatch"
							style:background={row.color}
							aria-hidden="true"
						></span>{/if}
					<span class={['text', 'truncate', row.mono && 'mono']} title={row.label}>{row.label}</span
					>
					{#if row.hint}<span class="hint">{row.hint}</span>{/if}
				</span>
				<span class="value num">{row.display ?? format(row.value)}</span>
				<span class="track" aria-hidden="true">
					<span
						class="bar"
						style:width="{top ? (row.value / top) * 100 : 0}%"
						style:background={row.color ?? color}
					></span>
				</span>
			</svelte:element>
		</li>
	{/each}
</ul>

<style>
	.bar-list {
		display: grid;
		gap: var(--space-1);
	}

	.row {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		align-items: center;
		gap: var(--space-1) var(--space-3);
		padding: var(--space-1_5) var(--space-2);
		margin: 0 calc(-1 * var(--space-2));
		border-radius: var(--radius-sm);
		color: inherit;
		text-decoration: none;
	}

	a.row:hover {
		background: var(--color-surface-hover);
	}

	.name {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-width: 0;
		font-size: var(--text-sm);
	}

	.text {
		min-width: 0;
	}

	.text.mono {
		font-size: var(--text-xs);
	}

	.hint {
		flex: none;
		color: var(--color-text-muted);
		font-size: var(--text-xs);
	}

	.swatch {
		flex: none;
		width: 8px;
		height: 8px;
		border-radius: 2px;
	}

	.value {
		color: var(--color-text);
		font-size: var(--text-sm);
		font-weight: var(--weight-medium);
	}

	.track {
		grid-column: 1 / -1;
		height: 6px;
		border-radius: var(--radius-full);
		background: var(--color-surface-sunken);
	}

	.bar {
		display: block;
		min-width: 2px;
		height: 100%;
		border-radius: 0 var(--radius-full) var(--radius-full) 0;
		transition: width var(--duration-slow) var(--ease-out);
	}
</style>
