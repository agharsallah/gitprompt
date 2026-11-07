<script lang="ts" module>
	import type { Outcome } from '#lib/insights/outcome.ts';

	export type LaneMark = { id: number; at: number; outcome: Outcome; text: string };
	export type LaneRow = { key: string; label: string; href?: string; marks: LaneMark[] };
</script>

<script lang="ts">
	import Legend from '#lib/ui/charts/Legend.svelte';
	import Tooltip from '#lib/ui/charts/Tooltip.svelte';
	import { OUTCOMES } from '#lib/insights/outcome.ts';
	import { excerpt, integer, timeOfDay } from '#lib/shared/format.ts';

	/**
	 * Swimlanes on one time axis: a row per lane (project), a tick per item (prompt) where it
	 * happened, its color and shape stating the outcome. Shows where and when work went at a glance.
	 */
	let {
		lanes,
		start,
		end,
		label,
		hrefFor
	}: {
		lanes: LaneRow[];
		start: number;
		end: number;
		label: string;
		/** Where clicking a mark leads. */
		hrefFor: (id: number) => string;
	} = $props();

	const span = $derived(Math.max(end - start, 1));
	const left = (t: number) => `${((t - start) / span) * 100}%`;

	const HOUR = 3_600_000;
	const DAY = 24 * HOUR;
	/** Axis ticks on local hour/day boundaries, about six of them. */
	const ticks = $derived.by(() => {
		const step = [
			HOUR,
			3 * HOUR,
			6 * HOUR,
			12 * HOUR,
			DAY,
			2 * DAY,
			7 * DAY,
			14 * DAY,
			30 * DAY
		].find((s) => span / s <= 7)!;
		const first = new Date(start);
		if (step < DAY) first.setMinutes(0, 0, 0);
		else first.setHours(0, 0, 0, 0);
		const out: { t: number; text: string }[] = [];
		for (let t = first.getTime(); t <= end && out.length < 40;) {
			if (t >= start) {
				const d = new Date(t);
				out.push({
					t,
					text:
						step < DAY
							? d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
							: d.toLocaleDateString('en-GB', { weekday: 'short', day: 'numeric' })
				});
			}
			// Step in calendar units for days so daylight-saving changes don't drift the ticks.
			const next = new Date(t);
			if (step < DAY) next.setTime(t + step);
			else next.setDate(next.getDate() + step / DAY);
			t = next.getTime();
		}
		return out;
	});

	let width = $state(0);
	let active = $state<{ mark: LaneMark; x: number; y: number } | null>(null);

	function show(event: PointerEvent | FocusEvent, mark: LaneMark) {
		const el = event.currentTarget as HTMLElement;
		const track = el.parentElement!;
		active = { mark, x: track.offsetLeft + el.offsetLeft + 1, y: track.offsetTop };
	}

	const LEGEND = (['done', 'failed', 'interrupted', 'running', 'open'] as const).map((o) => ({
		label: OUTCOMES[o].label,
		color: `var(--lane-${o})`,
		shape: 'tick' as const
	}));
</script>

<figure class="lanes" bind:clientWidth={width} aria-label={label}>
	<div class="rows">
		{#each lanes as lane (lane.key)}
			{@const failed = lane.marks.filter((m) => m.outcome === 'failed').length}
			<div class="lane">
				<div class="lane-label">
					{#if lane.href}
						<a href={lane.href} class="truncate" title={lane.label}>{lane.label}</a>
					{:else}
						<span class="truncate" title={lane.label}>{lane.label}</span>
					{/if}
					<span
						class="count num"
						aria-label="{integer(lane.marks.length)} prompts{failed ? `, ${failed} failed` : ''}"
					>
						{integer(lane.marks.length)}
					</span>
				</div>
				<div class="track">
					{#each ticks as tick (tick.t)}<span class="gridline" style:left={left(tick.t)}
						></span>{/each}
					{#each lane.marks as mark (mark.id)}
						<a
							class="mark {mark.outcome}"
							href={hrefFor(mark.id)}
							style:left={left(mark.at)}
							tabindex="-1"
							aria-label="{OUTCOMES[mark.outcome].label} at {timeOfDay(
								new Date(mark.at).toISOString()
							)}: {excerpt(mark.text, 80)}"
							onpointerenter={(e) => show(e, mark)}
							onpointerleave={() => (active = null)}
						></a>
					{/each}
				</div>
			</div>
		{/each}
		<div class="lane" aria-hidden="true">
			<span></span>
			<div class="axis">
				{#each ticks as tick (tick.t)}<span style:left={left(tick.t)}>{tick.text}</span>{/each}
			</div>
		</div>
	</div>
	{#if active}
		<Tooltip x={active.x} y={active.y} {width}>
			<strong>{excerpt(active.mark.text, 90) || 'No prompt text'}</strong>
			{new Date(active.mark.at).toLocaleString([], {
				weekday: 'short',
				hour: '2-digit',
				minute: '2-digit'
			})}, {OUTCOMES[active.mark.outcome].label.toLowerCase()}
		</Tooltip>
	{/if}
	<Legend items={LEGEND} label="Outcome" />
</figure>

<style>
	.lanes {
		--lane-done: var(--color-accent);
		--lane-failed: var(--color-danger-solid);
		--lane-interrupted: var(--color-info-solid);
		--lane-running: var(--color-agent);
		--lane-open: var(--color-border-control);

		position: relative;
		display: grid;
		gap: var(--space-3);
		min-width: 0;
		margin: 0;
	}

	.rows {
		display: grid;
		gap: var(--space-1);
	}

	.lane {
		display: grid;
		grid-template-columns: minmax(7rem, 11rem) minmax(0, 1fr);
		align-items: center;
		gap: var(--space-3);
	}

	.lane-label {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: var(--space-2);
		min-width: 0;
		font-size: var(--text-sm);
	}

	.lane-label a,
	.lane-label span.truncate {
		min-width: 0;
		color: var(--color-text);
		font-weight: var(--weight-medium);
		text-decoration: none;
	}

	.lane-label a:hover {
		color: var(--color-accent-text);
		text-decoration: underline;
	}

	.count {
		flex: none;
		color: var(--color-text-muted);
		font-size: var(--text-xs);
	}

	.track {
		position: relative;
		height: 28px;
		border-radius: var(--radius-sm);
		background: var(--color-surface-sunken);
	}

	.gridline {
		position: absolute;
		top: 0;
		bottom: 0;
		width: 1px;
		background: var(--chart-grid);
	}

	.mark {
		position: absolute;
		top: 6px;
		width: 3px;
		height: 16px;
		margin-left: -1px;
		border-radius: 2px;
		background: var(--lane-done);
	}

	/* A wider invisible hit area than the 3px mark. */
	.mark::before {
		content: '';
		position: absolute;
		inset: -6px -4px;
	}

	.mark:hover {
		z-index: 1;
		transform: scaleY(1.25);
	}

	.mark.failed {
		top: 4px;
		height: 20px;
		background: var(--lane-failed);
	}

	.mark.interrupted {
		background: var(--lane-interrupted);
	}

	.mark.running {
		background: var(--lane-running);
	}

	.mark.open {
		top: 9px;
		height: 10px;
		background: var(--lane-open);
	}

	.axis {
		position: relative;
		height: 1rem;
	}

	.axis span {
		position: absolute;
		color: var(--color-text-muted);
		font-size: 11px;
		white-space: nowrap;
		transform: translateX(-50%);
	}

	@media (max-width: 40rem) {
		.lane {
			grid-template-columns: minmax(0, 1fr);
			gap: var(--space-1);
		}
	}
</style>
