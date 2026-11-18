<script lang="ts">
	import type { Overview } from '#lib/insights/overview.ts';
	import { chartStart, hourKey, streaks, timeSlots } from '#lib/insights/series.ts';
	import {
		dayHref,
		standout,
		standoutLine,
		streakLine,
		stripColumns,
		stripReadout,
		stripSeries,
		type StripColumn
	} from '#lib/overview/week-strip.ts';
	import { compact, dayKey, plural } from '#lib/shared/format.ts';
	import { track } from '#lib/shared/usage.ts';
	import StackedColumns from '#lib/ui/charts/StackedColumns.svelte';

	/**
	 * The range as one short strip: prompts per day (per clock hour over 24 hours), stacked by
	 * project, today outlined and the day that stands out marked. A day opens its sessions; a
	 * project in the legend, hovered or pressed, dims the others.
	 */
	let { data }: { data: Overview } = $props();

	const now = $derived(new Date(data.now));
	const hourly = $derived(data.hourly);
	const slots = $derived(
		timeSlots(
			data.strip.groups,
			chartStart(data.start, data.strip.totals.first_at, data.now),
			now,
			hourly
		)
	);
	const series = $derived(stripSeries(data.strip.groups));
	const columns = $derived(stripColumns(slots, series, hourly, now));
	const marked = $derived(standout(columns));
	const today = $derived(hourly ? hourKey(now) : dayKey(now));
	const cost = $derived(data.strip.totals.cost_usd > 0);

	let hovered = $state<string | null>(null);
	let pinned = $state<string | null>(null);
	const highlight = $derived(hovered ?? pinned);

	const lines = (c: StripColumn) => stripReadout(c, series, cost);
	const describe = (c: StripColumn) => [c.label, ...lines(c)].join(', ');
</script>

<section class="strip" aria-labelledby="week-strip">
	<header>
		<h2 id="week-strip">{hourly ? 'Each hour, by project' : 'Each day, by project'}</h2>
		{#if !hourly}<p class="note">{streakLine(streaks(slots))}</p>{/if}
	</header>
	<StackedColumns
		data={columns}
		{series}
		label={hourly ? 'Prompts per hour, by project' : 'Prompts per day, by project'}
		format={(v) => plural(v, 'prompt')}
		axisFormat={compact}
		outline={today}
		mark={marked?.key}
		{highlight}
		hrefFor={(c) => dayHref(c, data.filters.repo)}
		{describe}
		onopen={() => track('week.day.open')}
	>
		{#snippet readout(c)}
			<strong>{c.label}</strong>
			{#each lines(c) as line (line)}<span class="line">{line}</span>{/each}
		{/snippet}
	</StackedColumns>
	<footer>
		{#if series.length > 1}
			<ul class="legend" aria-label="Projects">
				{#each series as s (s.key)}
					<li>
						<button
							type="button"
							class={['key', highlight && highlight !== s.key && 'dim']}
							aria-pressed={pinned === s.key}
							onpointerenter={() => (hovered = s.key)}
							onpointerleave={() => (hovered = null)}
							onfocus={() => (hovered = s.key)}
							onblur={() => (hovered = null)}
							onclick={() => (pinned = pinned === s.key ? null : s.key)}
						>
							<span class="swatch" style:--key={s.color} aria-hidden="true"></span>{s.label}
						</button>
					</li>
				{/each}
			</ul>
		{/if}
		{#if marked}<p class="note">{standoutLine(columns, marked, hourly)}</p>{/if}
	</footer>
</section>

<style>
	.strip {
		display: grid;
		gap: var(--space-2);
		min-width: 0;
	}

	header,
	footer {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--space-1) var(--space-4);
	}

	h2 {
		color: var(--color-text);
		font-size: var(--text-sm);
		font-weight: var(--weight-semibold);
	}

	.note {
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
	}

	.line {
		display: block;
	}

	.legend {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1) var(--space-3);
	}

	.key {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1_5);
		padding: 0 var(--space-1);
		border-radius: var(--radius-sm);
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
		transition: opacity var(--duration-fast) var(--ease-out);
	}

	.key[aria-pressed='true'] {
		color: var(--color-text);
		font-weight: var(--weight-medium);
	}

	.key.dim {
		opacity: 0.45;
	}

	.swatch {
		width: 10px;
		height: 10px;
		border-radius: 3px;
		background: var(--key);
	}
</style>
