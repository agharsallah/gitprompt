<script lang="ts">
	import type { StatsBucket } from '#lib/api/types.ts';
	import type { Doorway } from '#lib/insights/filters.ts';
	import type { Overview } from '#lib/insights/overview.ts';
	import { cacheShare, tokenTotal } from '#lib/insights/series.ts';
	import { compact, integer, modelName, money, percent, plural } from '#lib/shared/format.ts';
	import BarList from '#lib/ui/charts/BarList.svelte';
	import Meter from '#lib/ui/charts/Meter.svelte';
	import Panel from '#lib/ui/Panel.svelte';
	import SegmentedControl from '#lib/ui/SegmentedControl.svelte';
	import type { Availability } from '#lib/sources/capabilities.ts';
	import CapabilityNote from './CapabilityNote.svelte';
	import NotAvailable from './NotAvailable.svelte';

	/**
	 * Which model does the work. "Most used" depends on the measure: a small model can serve the most
	 * requests while a large one costs the most, so the measures are one switch apart. Tools whose
	 * hooks report the model but no usage (Codex) count prompts only, so without any requests the
	 * panel measures prompts.
	 */
	let { data, link, availability }: { data: Overview; link: Doorway; availability: Availability } =
		$props();

	const MEASURES = {
		prompts: { label: 'Prompts', value: (b: StatsBucket) => b.prompts, format: compact },
		requests: { label: 'Requests', value: (b: StatsBucket) => b.api_requests, format: compact },
		tokens: { label: 'Tokens', value: tokenTotal, format: compact },
		cost: { label: 'Cost', value: (b: StatsBucket) => b.cost_usd, format: money }
	} as const;
	type Measure = keyof typeof MEASURES;
	const models = $derived(data.models.groups.filter((g) => g.key));
	const usage = $derived(models.some((g) => g.api_requests));
	/** Prompts are the only measure without usage; with it, requests come first. */
	const offered = $derived<Measure[]>(
		usage ? ['requests', 'tokens', 'cost', 'prompts'] : ['prompts']
	);
	let chosen = $state<Measure>('requests');
	const measure = $derived(offered.includes(chosen) ? chosen : offered[0]);
	const m = $derived(MEASURES[measure]);
	const sorted = $derived(
		models.filter((g) => m.value(g) > 0).sort((a, b) => m.value(b) - m.value(a))
	);
	const top = (key: Measure) =>
		[...models].sort((a, b) => MEASURES[key].value(b) - MEASURES[key].value(a))[0]?.key;
	const leaders = $derived(offered.map((k) => ({ k, model: top(k) })));
	const cache = $derived(cacheShare(data.activity.totals));
	const describe = $derived(
		!models.length
			? undefined
			: !usage
				? `Most prompts: ${modelName(leaders[0].model ?? '')}.`
				: leaders[0].model !== top('cost')
					? `Most used by requests: ${modelName(leaders[0].model ?? '')}; by cost: ${modelName(top('cost') ?? '')}.`
					: `${modelName(leaders[0].model ?? '')} leads on requests and cost.`
	);
</script>

<Panel title="Which model do I use most?" description={describe}>
	{#snippet actions()}
		{#if offered.length > 1}
			<SegmentedControl
				label="Measure"
				size="sm"
				bind:value={() => measure, (v) => (chosen = v)}
				options={offered.map((k) => ({ value: k, label: MEASURES[k].label }))}
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
					hint: usage
						? `${integer(g.api_requests)} requests · ${compact(tokenTotal(g))} tokens · ${money(g.cost_usd)}`
						: plural(g.prompts, 'prompt'),
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
			<CapabilityNote {availability} />
		</div>
	{:else}
		<NotAvailable what="models" {availability} />
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
