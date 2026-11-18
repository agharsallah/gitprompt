<script lang="ts">
	import { CircleX, Layers, ServerCrash, Wrench } from '@lucide/svelte';
	import type { Doorway } from '#lib/insights/filters.ts';
	import type { Overview } from '#lib/insights/overview.ts';
	import { ratio } from '#lib/insights/series.ts';
	import { duration, integer, percent, plural } from '#lib/shared/format.ts';
	import SegmentedBar from '#lib/ui/charts/SegmentedBar.svelte';
	import { countPanel } from '#lib/insights/counting.ts';
	import Panel from '#lib/ui/Panel.svelte';
	import { readyOr } from '#lib/ui/panelState.ts';
	import Facts from './Facts.svelte';

	/** How fast turns are, where the time goes (model vs tools), and what went wrong. */
	let { data, link }: { data: Overview; link: Doorway } = $props();

	const t = $derived(data.activity.totals);
	const outcome = (key: string) => data.outcomes.groups.find((g) => g.key === key)?.prompts ?? 0;
	const failed = $derived(outcome('failed'));
	const done = $derived(outcome('done'));
	const timed = $derived(t.api_time_s + t.tool_time_s > 0);
	const view = $derived(
		readyOr(t.ended, {
			kind: 'empty',
			title: 'No finished turn yet',
			message: "A turn's time is known once the tool reports its end (Stop)."
		})
	);
</script>

<Panel
	{@attach countPanel('performance')}
	state={view}
	title="How fast and reliable are turns?"
	description={t.ended ? `Over ${plural(t.ended, 'finished turn')}.` : undefined}
>
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
			<div>
				<dt>Finished</dt>
				<dd>{done + failed ? percent(ratio(done, done + failed)) : '—'}</dd>
				<dd class="note">{failed ? `${plural(failed, 'turn')} failed` : 'No failed turns'}</dd>
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
				Summed durations: calls that run in parallel (subagents) add up, so the total can exceed the
				turns' length.
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
</Panel>

<style>
	.stack {
		display: grid;
		gap: var(--space-4);
	}

	.speed {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(7rem, 1fr));
		gap: var(--space-3);
	}

	dt {
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}

	dd:not(.note) {
		font-size: var(--text-xl);
		font-weight: var(--weight-semibold);
	}

	.note {
		color: var(--color-text-muted);
		font-size: var(--text-xs);
	}
</style>
