<script lang="ts">
	import { AtSign, Code, Link } from '@lucide/svelte';
	import type { Doorway } from '#lib/insights/filters.ts';
	import type { Overview } from '#lib/insights/overview.ts';
	import { bands, SIZES } from '#lib/insights/series.ts';
	import {
		compact,
		duration,
		integer,
		percent,
		plural,
		shortDuration
	} from '#lib/shared/format.ts';
	import { durationTicks } from '#lib/ui/charts/scale.ts';
	import ColumnChart from '#lib/ui/charts/ColumnChart.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import Panel from '#lib/ui/Panel.svelte';
	import Facts from './Facts.svelte';

	/** How long typed prompts are, how long their turns take, and what they reference. */
	let { data, link }: { data: Overview; link: Doorway } = $props();

	const t = $derived(data.activity.totals);
	const sizes = $derived(bands(data.sizes.groups, SIZES, integer));
	const share = (n: number) => `${percent(n / t.prompts)} (${integer(n)})`;
</script>

<Panel
	title="How long are my prompts, and what's in them?"
	description="Typed characters, without the context the tool added."
>
	{#if t.prompts}
		<div class="stack">
			<ColumnChart
				label="Prompts by typed length"
				data={sizes.map((b) => ({
					key: b.key,
					label: `${b.label} characters`,
					tick: b.tick,
					value: b.bucket?.prompts ?? 0
				}))}
				height={96}
				format={(v) => plural(v, 'prompt')}
				axisFormat={compact}
				hrefFor={(c) => link({ size: c.key })}
			/>
			<h3 class="sub">Typical turn, by prompt length</h3>
			<ColumnChart
				label="Median turn duration by prompt length"
				data={sizes.map((b) => ({
					key: b.key,
					label: `${b.label} characters (${plural(b.bucket?.ended ?? 0, 'finished turn')})`,
					tick: b.tick,
					value: b.bucket?.response_s_p50 ?? 0
				}))}
				height={80}
				format={(v) => duration(v)}
				axisFormat={shortDuration}
				ticks={durationTicks(Math.max(0, ...sizes.map((b) => b.bucket?.response_s_p50 ?? 0)))}
				color="var(--chart-2)"
				hrefFor={(c) => link({ size: c.key })}
			/>
			<Facts
				label="What prompts reference"
				facts={[
					{
						label: 'Include a code block',
						value: share(t.with_code),
						icon: Code,
						href: t.with_code ? link({ has: ['code'] }) : undefined
					},
					{
						label: 'Link somewhere',
						value: share(t.with_url),
						icon: Link,
						href: t.with_url ? link({ has: ['url'] }) : undefined
					},
					{
						label: '@-mention a file',
						value: share(t.with_mention),
						icon: AtSign,
						href: t.with_mention ? link({ has: ['mention'] }) : undefined
					}
				]}
			/>
		</div>
	{:else}
		<EmptyState compact title="No prompts in this window" />
	{/if}
</Panel>

<style>
	.stack {
		display: grid;
		gap: var(--space-3);
	}

	.sub {
		margin-top: var(--space-2);
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
		font-weight: var(--weight-medium);
		font-stretch: var(--stretch-normal);
		letter-spacing: var(--tracking-normal);
	}
</style>
