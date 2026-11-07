<script lang="ts">
	import { CircleX, Layers, ServerCrash, Wrench } from '@lucide/svelte';
	import type { Doorway } from '#lib/insights/filters.ts';
	import type { Overview } from '#lib/insights/overview.ts';
	import { duration, integer, plural } from '#lib/shared/format.ts';
	import SegmentedBar from '#lib/ui/charts/SegmentedBar.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import Panel from '#lib/ui/Panel.svelte';
	import Facts from './Facts.svelte';

	/** How fast turns are, where the time goes (model vs tools), and what went wrong. */
	let { data, link }: { data: Overview; link: Doorway } = $props();

	const t = $derived(data.activity.totals);
	const failed = $derived(data.outcomes.groups.find((g) => g.key === 'failed')?.prompts ?? 0);
	const timed = $derived(t.api_time_s + t.tool_time_s > 0);
</script>

<Panel
	title="How fast and reliable are turns?"
	description={t.ended ? `Over ${plural(t.ended, 'finished turn')}.` : undefined}
>
	{#if t.ended}
		<div class="stack">
			<dl class="speed">
				<div>
					<dt>Typical turn</dt>
					<dd>{duration(t.response_s_p50)}</dd>
				</div>
				<div>
					<dt>1 in 10 take over</dt>
					<dd>{duration(t.response_s_p90)}</dd>
				</div>
			</dl>
			{#if timed}
				<SegmentedBar
					variant="tall"
					label="Time in model calls and in tools"
					parts={[
						{ key: 'model', label: 'Model calls', value: t.api_time_s, color: 'var(--chart-1)' },
						{ key: 'tools', label: 'Tools', value: t.tool_time_s, color: 'var(--chart-2)' }
					]}
					format={(v) => duration(v)}
				/>
				<p class="note">
					Summed durations: calls that run in parallel (subagents) add up, so the total can exceed
					the turns' length.
				</p>
			{/if}
			<Facts
				label="What went wrong"
				facts={[
					{
						label: 'Turns that failed',
						value: integer(failed),
						icon: CircleX,
						hint: 'ended with StopFailure',
						href: failed ? link({ outcome: ['failed'] }) : undefined
					},
					{
						label: 'Tool calls that failed',
						value: integer(t.tool_failures),
						icon: Wrench,
						href: t.tool_failures ? link({ has: ['tool_failure'] }) : undefined
					},
					{
						label: 'API errors',
						value: integer(t.api_errors),
						icon: ServerCrash,
						hint: 'telemetry',
						href: t.api_errors ? link({ has: ['api_error'] }) : undefined
					},
					{
						label: 'Compactions',
						value: integer(t.compactions),
						icon: Layers,
						hint: 'the context filled up',
						href: t.compactions ? link({ has: ['compaction'] }) : undefined
					}
				]}
			/>
		</div>
	{:else}
		<EmptyState compact title="No finished turn yet"
			>A turn's time is known once the tool reports its end (Stop).</EmptyState
		>
	{/if}
</Panel>

<style>
	.stack {
		display: grid;
		gap: var(--space-4);
	}

	.speed {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: var(--space-3);
	}

	dt {
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}

	dd {
		font-size: var(--text-xl);
		font-weight: var(--weight-semibold);
	}

	.note {
		color: var(--color-text-muted);
		font-size: var(--text-xs);
	}
</style>
