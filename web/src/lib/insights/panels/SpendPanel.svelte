<script lang="ts">
	import { folder, money, plural } from '#lib/shared/format.ts';
	import type { Doorway } from '#lib/insights/filters.ts';
	import type { Overview } from '#lib/insights/overview.ts';
	import { WORK, WORK_INFO } from '#lib/insights/series.ts';
	import BarList from '#lib/ui/charts/BarList.svelte';
	import SegmentedBar from '#lib/ui/charts/SegmentedBar.svelte';
	import Panel from '#lib/ui/Panel.svelte';
	import Facts from './Facts.svelte';
	import type { Availability } from '#lib/sources/capabilities.ts';
	import CapabilityNote from './CapabilityNote.svelte';
	import NotAvailable from './NotAvailable.svelte';

	/**
	 * Who the money was spent for (telemetry `query_source`): answering you, subagents, or calls the
	 * tool makes on its own; then per prompt and per project.
	 */
	let { data, link, availability }: { data: Overview; link: Doorway; availability: Availability } =
		$props();

	const t = $derived(data.activity.totals);
	const cost = (key: string) => data.work.groups.find((g) => g.key === key)?.cost_usd ?? 0;
	const projects = $derived(data.repos.groups.filter((g) => g.cost_usd > 0));
</script>

<Panel
	title="Where does the money go?"
	description={t.cost_usd ? `${money(t.cost_usd)} over ${plural(t.prompts, 'prompt')}.` : undefined}
>
	{#if t.cost_usd}
		<div class="stack">
			<SegmentedBar
				variant="tall"
				label="Cost by who the model worked for"
				parts={WORK.map((w) => ({
					key: w,
					label: WORK_INFO[w].label,
					value: cost(w),
					color: WORK_INFO[w].color
				}))}
				format={money}
			/>
			<Facts
				label="Cost per prompt"
				facts={[
					{
						label: 'Per prompt',
						value: money(t.cost_usd / t.prompts),
						hint: 'everything, divided by the prompts kept'
					},
					{ label: 'Answering you, per prompt', value: money(cost('main') / t.prompts) },
					{
						label: 'Subagent work',
						value: money(cost('subagent')),
						hint: WORK_INFO.subagent.hint,
						href: t.subagents ? link({ has: ['subagent'] }) : undefined
					},
					{ label: 'System calls', value: money(cost('system')), hint: WORK_INFO.system.hint }
				]}
			/>
			<h3 class="sub">By project</h3>
			<BarList
				label="Cost by project"
				max={6}
				otherLabel="Other projects"
				items={projects.map((g) => ({
					key: g.key ?? '(none)',
					label: g.detail ?? folder(g.key) ?? 'No folder',
					value: g.cost_usd,
					hint: `${money(g.cost_usd / g.prompts)} per prompt`,
					href: g.key ? link({ repo: g.key }) : undefined
				}))}
				format={money}
			/>
			<CapabilityNote {availability} />
		</div>
	{:else}
		<NotAvailable what="costs" {availability} />
	{/if}
</Panel>

<style>
	.stack {
		display: grid;
		gap: var(--space-4);
	}

	.sub {
		margin-bottom: calc(-1 * var(--space-2));
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
		font-weight: var(--weight-medium);
		font-stretch: var(--stretch-normal);
		letter-spacing: var(--tracking-normal);
	}
</style>
