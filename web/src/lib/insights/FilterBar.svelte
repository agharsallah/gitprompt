<script lang="ts">
	import { X } from '@lucide/svelte';
	import { page } from '$app/state';
	import type { Origin, Stats } from '#lib/api/types.ts';
	import { DIMENSIONS, type Dimension, type InsightFilters } from '#lib/insights/filters.ts';
	import FacetSelects from '#lib/insights/FacetSelects.svelte';
	import RangePicker from '#lib/insights/RangePicker.svelte';
	import OriginChips from '#lib/prompts/OriginChips.svelte';
	import { originFilter, parseOrigins } from '#lib/prompts/originFilter.svelte.ts';
	import Button from '#lib/ui/Button.svelte';

	/**
	 * The Overview's filters in one row above the charts: time range, origins, tool, app, project and
	 * model. A plain GET form, so the URL holds the view (linkable) and the page's load refetches.
	 * Each choice is counted without its own filter. The page fills in the filters last used in this
	 * browser when the URL has none (lib/settings/remembered.ts), so "Clear filters" sets each one to
	 * "every …" explicitly rather than leaving it out.
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
	const submit = () => form.requestSubmit();

	/** Remember the new origin choice for every prompt view, then load it. */
	function changeOrigins() {
		originFilter.set(parseOrigins(new FormData(form).getAll('origin').map(String)));
		submit();
	}

	const counts = $derived(
		Object.fromEntries(origins.groups.map((g) => [g.key, g.prompts])) as Partial<
			Record<Origin, number>
		>
	);
	const range = $derived(page.url.searchParams.get('range'));
	const filtered = $derived(DIMENSIONS.some((d) => filters[d]));
	const cleared = $derived.by(() => {
		const params = new URLSearchParams(page.url.search);
		for (const d of DIMENSIONS) params.set(d, '');
		return `${page.url.pathname}?${params}`;
	});
</script>

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
	{#if filtered}
		<Button href={cleared} size="sm" variant="ghost" icon={X}>Clear filters</Button>
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
</style>
