<script lang="ts">
	import type { Origin, StatsBucket } from '#lib/api/types.ts';
	import { originOf } from '#lib/prompts/origin.ts';
	import { integer } from '#lib/shared/format.ts';

	/** A connector's prompts by who produced them (GET /api/stats?source=…&group_by=origin). */
	let { groups }: { groups: StatsBucket[] } = $props();

	const rows = $derived(
		groups.map((g) => ({ ...originOf({ origin: (g.key ?? 'unknown') as Origin }), n: g.prompts }))
	);
</script>

{#if rows.length}
	<div class="origins">
		<span class="muted">Prompts by origin</span>
		<ul>
			{#each rows as row (row.label)}
				{@const Icon = row.icon}
				<li class={[row.system && 'system']} title={row.hint}>
					<Icon size={13} aria-hidden="true" /><span class="n">{integer(row.n)}</span>
					{row.label.toLowerCase()}
				</li>
			{/each}
		</ul>
	</div>
{/if}

<style>
	.origins {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: var(--space-1) var(--space-3);
		font-size: var(--text-sm);
	}

	ul {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1) var(--space-3);
	}

	li {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
	}

	li :global(svg) {
		color: var(--color-text-muted);
	}

	.system {
		color: var(--color-text-secondary);
	}

	.n {
		font-weight: var(--weight-semibold);
		font-variant-numeric: tabular-nums;
	}
</style>
