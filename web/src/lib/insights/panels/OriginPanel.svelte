<script lang="ts">
	import type { Origin } from '#lib/api/types.ts';
	import type { Doorway } from '#lib/insights/filters.ts';
	import type { Overview } from '#lib/insights/overview.ts';
	import { ORIGINS } from '#lib/prompts/origin.ts';
	import { percent, plural } from '#lib/shared/format.ts';
	import BarList from '#lib/ui/charts/BarList.svelte';
	import Panel from '#lib/ui/Panel.svelte';
	import { readyOr } from '#lib/ui/panelState.ts';
	import { NO_PROMPTS } from './states.ts';

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
	const view = $derived(readyOr(all, NO_PROMPTS));
</script>

<Panel
	state={view}
	title="Who wrote these prompts?"
	description={all
		? `${percent(yours / all)} came from you; the rest the tool or its agents generated. Every origin, whatever the origin filter.`
		: undefined}
>
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
</Panel>
