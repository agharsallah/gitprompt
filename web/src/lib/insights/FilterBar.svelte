<script lang="ts">
	import { SlidersHorizontal, X } from '@lucide/svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import type { Origin, Stats } from '#lib/api/types.ts';
	import { clearedHref, filterChips, foldLabel } from '#lib/insights/filterChips.ts';
	import { DIMENSIONS, type Dimension, type InsightFilters } from '#lib/insights/filters.ts';
	import FacetSelects from '#lib/insights/FacetSelects.svelte';
	import RangePicker from '#lib/insights/RangePicker.svelte';
	import OriginChips from '#lib/prompts/OriginChips.svelte';
	import { originFilter, parseOrigins } from '#lib/prompts/originFilter.svelte.ts';
	import { track } from '#lib/shared/usage.ts';
	import Button from '#lib/ui/Button.svelte';
	import Chip from '#lib/ui/Chip.svelte';

	/**
	 * The Overview's filters: the time range and the origins in sight (they change every number),
	 * the others (tool, app, project, model) folded into one "Filters · N" button. Every filter that
	 * is on shows as a removable chip beside it, so the button never hides one. A plain GET form, so
	 * the URL holds the view (linkable) and the page's load refetches. Each choice is counted without
	 * its own filter. "Clear all" keeps the range and origins.
	 */
	let {
		filters,
		origins,
		options
	}: {
		filters: InsightFilters;
		/** Prompts per origin, the other filters applied. */
		origins: Stats;
		options: Record<Dimension, Stats>;
	} = $props();

	let form: HTMLFormElement;
	let fold: HTMLDetailsElement;
	let opened = $state(false);
	const submit = () => form.requestSubmit();

	/** Remember the new origin choice for every prompt view, then load it. */
	function changeOrigins() {
		originFilter.set(parseOrigins(new FormData(form).getAll('origin').map(String)));
		submit();
	}

	function close() {
		opened = false;
		fold.querySelector('summary')?.focus();
	}

	/** Escape closes the open popover, back on its button. */
	function escape(event: KeyboardEvent) {
		if (opened && event.key === 'Escape') close();
	}

	/** A press outside the open popover closes it. */
	function outside(event: PointerEvent) {
		if (opened && event.target instanceof Node && !fold.contains(event.target)) opened = false;
	}

	const counts = $derived(
		Object.fromEntries(origins.groups.map((g) => [g.key, g.prompts])) as Partial<
			Record<Origin, number>
		>
	);
	const range = $derived(page.url.searchParams.get('range'));
	const chips = $derived(filterChips(filters, options, page.url));
</script>

<svelte:window onpointerdown={outside} onkeydown={escape} />

<form
	method="GET"
	action={page.url.pathname}
	bind:this={form}
	data-sveltekit-reset="false"
	data-sveltekit-noscroll
	class="bar"
	aria-label="Filter the overview"
>
	<RangePicker range={filters.range} />
	{#if range}<input type="hidden" name="range" value={range} />{/if}
	<OriginChips origins={filters.origins} {counts} onchange={changeOrigins} />
	<details
		class="fold"
		bind:this={fold}
		bind:open={opened}
		ontoggle={() => fold.open && track('filters.open')}
	>
		<summary class={['toggle', chips.length && 'on']}>
			<SlidersHorizontal size={14} aria-hidden="true" />{foldLabel(chips.length)}
		</summary>
		<div class="popover">
			<FacetSelects
				facets={DIMENSIONS}
				values={{
					source: filters.source,
					surface: filters.surface,
					repo: filters.repo,
					model: filters.model
				}}
				{options}
				onchange={submit}
			/>
		</div>
	</details>
	{#if chips.length}
		<ul class="chips" aria-label="Filters on">
			{#each chips as chip (chip.dimension)}
				<li>
					<Chip onremove={() => goto(chip.href, { reset: false })} removeLabel="Remove {chip.label}"
						>{chip.label}</Chip
					>
				</li>
			{/each}
		</ul>
		<Button href={clearedHref(page.url, filters)} size="sm" variant="ghost" icon={X}
			>Clear all</Button
		>
	{/if}
	<noscript><button type="submit">Apply</button></noscript>
</form>

<style>
	.bar {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2) var(--space-4);
	}

	.fold {
		position: relative;
	}

	.toggle {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1_5);
		height: var(--control-sm);
		padding: 0 var(--space-2);
		border: var(--border-hairline) solid var(--color-border-strong);
		border-radius: var(--radius-sm);
		background: var(--color-surface-raised);
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
		font-weight: var(--weight-medium);
		list-style: none;
		cursor: pointer;
		user-select: none;
	}

	.toggle::-webkit-details-marker {
		display: none;
	}

	.toggle:hover {
		background: var(--color-surface-hover);
		border-color: var(--color-border-control);
		color: var(--color-text);
	}

	.toggle.on {
		border-color: var(--color-accent-border);
		background: var(--color-accent-soft);
		color: var(--color-accent-text);
	}

	.popover {
		position: absolute;
		z-index: var(--z-dropdown);
		top: calc(100% + var(--space-1_5));
		left: 0;
		width: max-content;
		max-width: min(36rem, calc(100vw - 2 * var(--space-4)));
		padding: var(--space-3);
		border-radius: var(--radius-md);
		background: var(--color-surface-raised);
		box-shadow: var(--shadow-overlay);
	}

	.chips {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1_5);
	}
</style>
