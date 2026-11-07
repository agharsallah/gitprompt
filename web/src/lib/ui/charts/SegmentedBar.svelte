<script lang="ts" module>
	export type Part = { key: string; label: string; value: number; color: string };
</script>

<script lang="ts">
	import Legend from '#lib/ui/charts/Legend.svelte';
	import Tooltip from '#lib/ui/charts/Tooltip.svelte';
	import { percent } from '#lib/shared/format.ts';

	/**
	 * One whole split into parts, in the order given (keep categorical order for adjacency).
	 *   thin: 8px, no text; the text beside it carries the figures. Optionally decorative.
	 *   tall: 28px blocks, each share printed inside where it fits, legend of names below.
	 * Every part is reachable without hover: keyboard focus shows the tooltip, and each part's
	 * accessible name holds its name, amount and share.
	 */
	let {
		parts,
		label,
		variant = 'thin',
		legend = variant === 'tall',
		format = (v: number) => String(v),
		decorative = false
	}: {
		parts: Part[];
		label: string;
		variant?: 'thin' | 'tall';
		legend?: boolean;
		format?: (value: number) => string;
		/** Hide from assistive tech (when a table or text next to it already says the same). */
		decorative?: boolean;
	} = $props();

	const shown = $derived(parts.filter((p) => p.value > 0));
	const total = $derived(shown.reduce((a, p) => a + p.value, 0));
	let width = $state(0);
	let active = $state<{ part: Part; x: number } | null>(null);

	const share = (p: Part) => (total ? p.value / total : 0);
	const describe = (p: Part) => `${p.label}: ${format(p.value)}, ${percent(share(p))}`;
	// Print a share inside a tall block only when it fits with padding (never clip a label).
	const fits = (p: Part) => share(p) * width >= percent(share(p)).length * 7.5 + 16;

	function show(event: Event, part: Part) {
		const el = event.currentTarget as HTMLElement;
		active = { part, x: el.offsetLeft + el.offsetWidth / 2 };
	}
</script>

<div class={['segmented-bar', variant]}>
	<div
		class="track"
		bind:clientWidth={width}
		role={decorative ? undefined : 'list'}
		aria-label={decorative ? undefined : label}
		aria-hidden={decorative ? 'true' : undefined}
	>
		{#each shown as part (part.key)}
			<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
			<span
				class="part"
				role={decorative ? undefined : 'listitem'}
				style:flex-grow={part.value}
				style:--part={part.color}
				tabindex={decorative || variant === 'thin' ? undefined : 0}
				aria-label={decorative ? undefined : describe(part)}
				onpointerenter={(e) => show(e, part)}
				onpointerleave={() => (active = null)}
				onfocus={(e) => show(e, part)}
				onblur={() => (active = null)}
			>
				{#if variant === 'tall' && fits(part)}<span class="share">{percent(share(part))}</span>{/if}
			</span>
		{/each}
		{#if !shown.length}<span class="part empty"></span>{/if}
		{#if active}
			<Tooltip x={active.x} y={0} {width}>
				<strong>{format(active.part.value)}</strong>
				{active.part.label}, {percent(share(active.part))}
			</Tooltip>
		{/if}
	</div>
	{#if legend && shown.length}
		<Legend items={shown.map((p) => ({ label: p.label, color: p.color }))} label="{label} legend" />
	{/if}
</div>

<style>
	.segmented-bar {
		display: grid;
		gap: var(--space-2);
		min-width: 0;
	}

	.track {
		position: relative;
		display: flex;
		gap: 2px;
		width: 100%;
	}

	.part {
		display: flex;
		align-items: center;
		flex-basis: 0;
		min-width: 3px;
		background: var(--part);
	}

	.thin .track {
		height: 8px;
	}

	.thin .part {
		border-radius: var(--radius-full);
	}

	.tall .track {
		height: 28px;
	}

	.tall .part {
		padding: 0 var(--space-2);
		border-radius: 4px;
	}

	.part:hover,
	.part:focus-visible {
		filter: brightness(1.08);
	}

	.part:focus-visible {
		box-shadow: var(--focus-ring);
		border-radius: 4px;
	}

	.empty {
		flex-grow: 1;
		background: var(--color-surface-sunken);
	}

	.share {
		color: #fff;
		font-size: var(--text-xs);
		font-weight: var(--weight-semibold);
		white-space: nowrap;
	}
</style>
