<script lang="ts">
	import type { Phase } from '#lib/api/types.ts';
	import { compact, integer, money, percent, plural, shortDate } from '#lib/shared/format.ts';
	import ColumnChart from '#lib/ui/charts/ColumnChart.svelte';
	import Panel from '#lib/ui/Panel.svelte';
	import OutcomeMix from '#lib/stories/OutcomeMix.svelte';

	/**
	 * How the project evolved from phase to phase: prompts and cost per phase (two charts, one
	 * scale each), then a table of how the prompts were written and how they ended. Every column
	 * and row leads to its phase.
	 */
	let { phases }: { phases: Phase[] } = $props();

	const columns = (value: (p: Phase) => number) =>
		phases.map((p) => ({
			key: String(p.index),
			label: `Phase ${p.index}, from ${shortDate(p.first_at ?? '')}`,
			tick: String(p.index),
			value: value(p)
		}));
	const withCost = $derived(phases.some((p) => p.cost_usd > 0));
	const toPhase = (c: { key: string }) => `#phase-${c.key}`;
	const done = (p: Phase) => (p.prompts ? (p.outcomes.done ?? 0) / p.prompts : 0);
</script>

<Panel
	title="How it evolved"
	description="Each phase is a stretch of work: a new one starts after a pause of 3 hours or more, or when you move to another branch for good."
>
	<div class="charts">
		<figure>
			<figcaption>Prompts per phase</figcaption>
			<ColumnChart
				data={columns((p) => p.prompts)}
				label="Prompts per phase"
				height={96}
				format={(v) => plural(v, 'prompt')}
				axisFormat={compact}
				hrefFor={toPhase}
			/>
		</figure>
		{#if withCost}
			<figure>
				<figcaption>Cost per phase</figcaption>
				<ColumnChart
					data={columns((p) => p.cost_usd)}
					label="Cost per phase"
					height={96}
					format={money}
					axisFormat={(v) => '$' + compact(v)}
					color="var(--chart-2)"
					hrefFor={toPhase}
				/>
			</figure>
		{/if}
	</div>
	<div class="table-wrap">
		<table>
			<thead>
				<tr>
					<th scope="col">Phase</th>
					<th scope="col" class="num">Prompts</th>
					<th scope="col" class="num" title="Median characters you typed per prompt"
						>Typical length</th
					>
					<th scope="col" class="num" title="Prompts that reword an earlier one of their session"
						>Reworded</th
					>
					<th scope="col" class="num">Finished</th>
					<th scope="col" class="outcomes">How they ended</th>
				</tr>
			</thead>
			<tbody>
				{#each phases as p (p.index)}
					<tr>
						<th scope="row"><a href="#phase-{p.index}">Phase {p.index}</a></th>
						<td class="num">{integer(p.prompts)}</td>
						<td class="num">{integer(p.median_chars)} chars</td>
						<td class="num">{integer(p.retries)}</td>
						<td class="num">{percent(done(p))}</td>
						<td class="outcomes"
							><OutcomeMix
								outcomes={p.outcomes}
								label="How the prompts of phase {p.index} ended"
								words={false}
							/></td
						>
					</tr>
				{/each}
			</tbody>
		</table>
	</div>
</Panel>

<style>
	.charts {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(16rem, 1fr));
		gap: var(--space-6);
		margin-bottom: var(--space-5);
	}

	figure {
		margin: 0;
	}

	figcaption {
		margin-bottom: var(--space-2);
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}

	.table-wrap {
		overflow-x: auto;
	}

	table {
		width: 100%;
		border-collapse: collapse;
		font-size: var(--text-sm);
		font-variant-numeric: tabular-nums;
	}

	th,
	td {
		padding: var(--space-1_5) var(--space-3) var(--space-1_5) 0;
		border-bottom: var(--border-hairline) solid var(--color-border-subtle);
		text-align: left;
		white-space: nowrap;
	}

	thead th {
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
		font-weight: var(--weight-medium);
	}

	tbody th {
		font-weight: var(--weight-medium);
	}

	.num {
		text-align: right;
	}

	.outcomes {
		width: 30%;
		min-width: 8rem;
	}
</style>
