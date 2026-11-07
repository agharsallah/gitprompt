<script lang="ts">
	import { Sparkles } from '@lucide/svelte';
	import type { Event } from '#lib/api/types.ts';
	import { KIND_INFO } from '#lib/insights/tools.ts';
	import { turnTimeline, type Span, type TimelineRow } from '#lib/insights/timeline.ts';
	import { duration, plural, shortDuration } from '#lib/shared/format.ts';
	import Tooltip from '#lib/ui/charts/Tooltip.svelte';
	import { durationTicks } from '#lib/ui/charts/scale.ts';

	/**
	 * The turn in time: a row for model calls (the agent thinking) and one per kind of tool (the
	 * agent acting), each call a span from when it started to when it finished. Shows at a glance
	 * whether a turn was mostly thinking, reading, editing or waiting on commands.
	 */
	let { events, start, responseS }: { events: Event[]; start: string; responseS?: number } =
		$props();

	const timeline = $derived(turnTimeline(events, start, responseS));
	const ticks = $derived(durationTicks(timeline.end, 4).filter((t) => t <= timeline.end * 1.001));
	const pct = (s: number) => `${(s / timeline.end) * 100}%`;

	const ROW_INFO = (row: TimelineRow) =>
		row === 'model'
			? { label: 'Model calls', color: 'var(--color-agent)', icon: Sparkles }
			: KIND_INFO[row];

	let width = $state(0);
	let active = $state<{ span: Span; x: number; y: number } | null>(null);

	function show(event: PointerEvent, span: Span) {
		const el = event.currentTarget as HTMLElement;
		const track = el.parentElement!;
		active = { span, x: track.offsetLeft + el.offsetLeft + el.offsetWidth / 2, y: track.offsetTop };
	}

	const summary = $derived(
		timeline.rows
			.map((r) => `${ROW_INFO(r).label}: ${timeline.spans.filter((s) => s.row === r).length}`)
			.join('; ')
	);
</script>

{#if timeline.spans.length}
	<figure class="timeline" bind:clientWidth={width} aria-label="Turn timeline over {duration(timeline.end)}. {summary}">
		{#each timeline.rows as row (row)}
			{@const info = ROW_INFO(row)}
			{@const spans = timeline.spans.filter((s) => s.row === row)}
			<div class="row">
				<span class="label">
					<info.icon size={14} aria-hidden="true" style="color: {info.color}" />
					<span class="truncate">{info.label}</span>
					<span class="count num">{spans.length}</span>
				</span>
				<div class="track" style:--span={info.color}>
					{#each ticks as t (t)}<span class="grid" style:left={pct(t)}></span>{/each}
					{#each spans as span (span.id)}
						<span
							class={['span', span.failed && 'failed']}
							style:left={pct(span.start)}
							style:width={pct(span.end - span.start)}
							onpointerenter={(e) => show(e, span)}
							onpointerleave={() => (active = null)}
							aria-hidden="true"
						></span>
					{/each}
				</div>
			</div>
		{/each}
		<div class="row" aria-hidden="true">
			<span></span>
			<div class="axis">
				{#each ticks as t (t)}<span style:left={pct(t)}>{shortDuration(t)}</span>{/each}
			</div>
		</div>
		{#if active}
			<Tooltip x={active.x} y={active.y} {width}>
				<strong>{active.span.label}{active.span.failed ? ' failed' : ''}</strong>
				{#if active.span.detail}<span class="detail mono">{active.span.detail}</span>{/if}
				{duration(active.span.end - active.span.start)}, at {duration(active.span.start)}
			</Tooltip>
		{/if}
		<figcaption class="sr-only">{plural(timeline.spans.length, 'call')}. The Events tab lists each one.</figcaption>
	</figure>
{/if}

<style>
	.timeline {
		position: relative;
		display: grid;
		gap: var(--space-1);
		margin: 0;
	}

	.row {
		display: grid;
		grid-template-columns: 9.5rem minmax(0, 1fr);
		align-items: center;
		gap: var(--space-3);
	}

	.label {
		display: flex;
		align-items: center;
		gap: var(--space-1_5);
		min-width: 0;
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
	}

	.count {
		margin-left: auto;
		color: var(--color-text-muted);
	}

	.track {
		position: relative;
		height: 20px;
		border-radius: var(--radius-xs);
		background: var(--color-surface-sunken);
	}

	.grid {
		position: absolute;
		top: 0;
		bottom: 0;
		width: 1px;
		background: var(--chart-grid);
	}

	.span {
		position: absolute;
		top: 5px;
		min-width: 3px;
		height: 10px;
		border-radius: 2px;
		background: var(--span);
	}

	.span::before {
		content: '';
		position: absolute;
		inset: -5px -3px;
	}

	.span:hover {
		z-index: 1;
		outline: 2px solid var(--color-surface);
		filter: brightness(1.1);
	}

	.span.failed {
		top: 3px;
		height: 14px;
		background: var(--color-danger-solid);
	}

	.axis {
		position: relative;
		height: 1rem;
	}

	.axis span {
		position: absolute;
		color: var(--color-text-muted);
		font-size: 11px;
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
		transform: translateX(-50%);
	}

	.axis span:first-child {
		transform: none;
	}

	.detail {
		display: block;
		max-width: 16rem;
		margin: 2px 0;
		overflow: hidden;
		color: var(--color-text);
		font-size: var(--text-2xs);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	@media (max-width: 40rem) {
		.row {
			grid-template-columns: 6.5rem minmax(0, 1fr);
		}
	}
</style>
