<script lang="ts">
	import type { Source } from '#lib/api/types.ts';
	import { compact, plural } from '#lib/shared/format.ts';
	import RelativeTime from '#lib/ui/RelativeTime.svelte';
	import Sparkline from '#lib/ui/charts/Sparkline.svelte';

	/** Is anything arriving? The last event, and prompts + events per hour over the last day. */
	let { source }: { source: Source } = $props();

	const day = $derived(source.hourly.reduce((a, b) => a + b, 0));
	const telemetry = $derived(source.total === 0 && source.events > 0);
</script>

<dl class="heartbeat">
	<div>
		<dt>Last event</dt>
		<dd><RelativeTime iso={source.last_at} /></dd>
	</div>
	<div class="spark">
		<dt>Last 24 hours</dt>
		<dd>
			<Sparkline
				values={source.hourly}
				width={120}
				height={24}
				label="{plural(day, 'event')} received in the last 24 hours, one point per hour"
			/>
			<span>{compact(day)}</span>
		</dd>
	</div>
	{#if !telemetry}
		<div>
			<dt>Prompts</dt>
			<dd>{compact(source.total)}</dd>
		</div>
	{/if}
	<div>
		<dt>Events</dt>
		<dd>{compact(source.events)}</dd>
	</div>
</dl>

<style>
	.heartbeat {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2) var(--space-6);
		margin: 0;
	}

	.heartbeat div {
		display: grid;
		gap: 2px;
	}

	dt {
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
	}

	dd {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-height: 24px;
		margin: 0;
		font-weight: var(--weight-semibold);
		font-variant-numeric: tabular-nums;
	}
</style>
