<script lang="ts">
	import type { Origin } from '#lib/api/types.ts';
	import type { Doorway } from '#lib/insights/filters.ts';
	import type { Overview } from '#lib/insights/overview.ts';
	import { ORIGINS } from '#lib/prompts/origin.ts';
	import { percent, plural } from '#lib/shared/format.ts';
	import BarList from '#lib/ui/charts/BarList.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import Panel from '#lib/ui/Panel.svelte';

	/**
	 * Who wrote the prompts the tool reported: you, or the system (task notifications, agents...).
	 * Counted over every origin, whatever the origin filter, so the share is visible.
	 */
	let { data, link }: { data: Overview; link: Doorway } = $props();

	const all = $derived(data.origins.totals.prompts);
	const yours = $derived(
		data.origins.groups
			.filter((g) => g.key && !ORIGINS[g.key as Origin]?.system)
			.reduce((n, g) => n + g.prompts, 0)
	);
</script>

<Panel
	title="Who wrote these prompts?"
	description={all
		? `${percent(yours / all)} came from you; the rest the tool or its agents generated. Every origin, whatever the origin filter.`
		: undefined}
>
	{#if all}
		<BarList
			label="Prompts by origin"
			items={data.origins.groups.map((g) => {
				const info = ORIGINS[(g.key ?? 'unknown') as Origin] ?? ORIGINS.unknown;
				return {
					key: g.key ?? 'unknown',
					label: info.label,
					value: g.prompts,
					icon: info.icon,
					hint: info.system ? 'System' : undefined,
					color: info.system ? 'var(--chart-dim)' : undefined,
					href: link({ origin: [(g.key ?? 'unknown') as Origin] })
				};
			})}
			format={(v) => plural(v, 'prompt')}
		/>
	{:else}
		<EmptyState compact title="No prompts in this window" />
	{/if}
</Panel>
