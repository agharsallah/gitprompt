<script lang="ts">
	import type { StatsBucket } from '#lib/api/types.ts';
	import type { Doorway } from '#lib/insights/filters.ts';
	import type { Overview } from '#lib/insights/overview.ts';
	import { cacheShare, tokenTotal } from '#lib/insights/series.ts';
	import { compact, integer, modelName, money, percent } from '#lib/shared/format.ts';
	import BarList from '#lib/ui/charts/BarList.svelte';
	import Meter from '#lib/ui/charts/Meter.svelte';
	import Panel from '#lib/ui/Panel.svelte';
	import SegmentedControl from '#lib/ui/SegmentedControl.svelte';
	import NeedsTelemetry from './NeedsTelemetry.svelte';

	/**
	 * Which model does the work. "Most used" depends on the measure: a small model can serve the most
	 * requests while a large one costs the most, so the three measures are one switch apart.
	 */
	let { data, link, connected }: { data: Overview; link: Doorway; connected: boolean } = $props();

	const MEASURES = {
		requests: { label: 'Requests', value: (b: StatsBucket) => b.api_requests, format: compact },
		tokens: { label: 'Tokens', value: tokenTotal, format: compact },
		cost: { label: 'Cost', value: (b: StatsBucket) => b.cost_usd, format: money }
	} as const;
	type Measure = keyof typeof MEASURES;
	let measure = $state<Measure>('requests');

	const models = $derived(data.models.groups.filter((g) => g.key && g.api_requests));
	const m = $derived(MEASURES[measure]);
	const sorted = $derived([...models].sort((a, b) => m.value(b) - m.value(a)));
	const top = (key: Measure) =>
		[...models].sort((a, b) => MEASURES[key].value(b) - MEASURES[key].value(a))[0]?.key;
	const leaders = $derived((Object.keys(MEASURES) as Measure[]).map((k) => ({ k, model: top(k) })));
	const cache = $derived(cacheShare(data.activity.totals));
	const describe = $derived(
		new Set(leaders.map((l) => l.model)).size > 1
			? `Most used by requests: ${modelName(leaders[0].model ?? '')}; by cost: ${modelName(leaders[2].model ?? '')}.`
			: models.length
				? `${modelName(leaders[0].model ?? '')} leads on every measure.`
				: undefined
	);
</script>

<Panel title="Which model do I use most?" description={describe}>
	{#snippet actions()}
		{#if models.length}
			<SegmentedControl
				label="Measure"
				size="sm"
				bind:value={measure}
				options={(Object.keys(MEASURES) as Measure[]).map((k) => ({
					value: k,
					label: MEASURES[k].label
				}))}
			/>
		{/if}
	{/snippet}
	{#if models.length}
		<div class="stack">
			<BarList
				label="{m.label} by model"
				color="var(--color-agent)"
				items={sorted.map((g) => ({
					key: g.key!,
					label: modelName(g.key!),
					value: m.value(g),
					hint: `${integer(g.api_requests)} requests · ${compact(tokenTotal(g))} tokens · ${money(g.cost_usd)}`,
					href: link({ model: [g.key!] })
				}))}
				format={m.format}
			/>
			{#if cache !== null}
				<div class="cache">
					<p><span class="num">{percent(cache)}</span> of prompt tokens were served from cache</p>
					<Meter value={cache} label="Prompt tokens served from cache" />
				</div>
			{/if}
		</div>
	{:else}
		<NeedsTelemetry what="models" {connected} />
	{/if}
</Panel>

<style>
	.stack {
		display: grid;
		gap: var(--space-4);
	}

	.cache {
		display: grid;
		gap: var(--space-2);
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}

	.num {
		color: var(--color-text);
		font-weight: var(--weight-semibold);
	}
</style>
