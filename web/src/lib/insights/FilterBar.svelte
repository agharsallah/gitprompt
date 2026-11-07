<script lang="ts">
	import { Bot, FolderGit2, Plug } from '@lucide/svelte';
	import { page } from '$app/state';
	import type { Origin, Stats } from '#lib/api/types.ts';
	import type { Dimension, InsightFilters } from '#lib/insights/filters.ts';
	import RangePicker from '#lib/insights/RangePicker.svelte';
	import type { RangeId } from '#lib/insights/range.ts';
	import OriginChips from '#lib/prompts/OriginChips.svelte';
	import { originFilter, parseOrigins } from '#lib/prompts/originFilter.svelte.ts';
	import { folder, modelName } from '#lib/shared/format.ts';
	import Select from '#lib/ui/Select.svelte';

	/**
	 * The Overview's filters in one row above the charts: time range, origins, source, project and
	 * model. A plain GET form, so the URL holds the view (linkable) and the page's load refetches.
	 * Each choice is counted without its own filter.
	 */
	let {
		filters,
		fallback,
		origins,
		options
	}: {
		filters: InsightFilters;
		/** The range shown without ?range= (the default_range setting). */
		fallback: RangeId;
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
	const NAMES: Record<Dimension, (key: string, detail: string | null) => string> = {
		source: (k) => k,
		repo: (k, detail) => detail ?? folder(k) ?? k,
		model: (k) => modelName(k)
	};
	const choices = (d: Dimension) => {
		const list = options[d].groups
			.filter((g) => g.key)
			.map((g) => ({ value: g.key!, label: `${NAMES[d](g.key!, g.detail)} (${g.prompts})` }));
		const chosen = filters[d];
		return chosen && !list.some((o) => o.value === chosen)
			? [{ value: chosen, label: NAMES[d](chosen, null) }, ...list]
			: list;
	};
	const range = $derived(page.url.searchParams.get('range'));
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
	<RangePicker range={filters.range} {fallback} />
	{#if range}<input type="hidden" name="range" value={range} />{/if}
	<OriginChips origins={filters.origins} {counts} onchange={changeOrigins} />
	<div class="selects">
		<Select
			label="Source"
			hideLabel
			name="source"
			icon={Plug}
			anyLabel="Every source"
			value={filters.source ?? ''}
			options={choices('source')}
			onchange={submit}
		/>
		<Select
			label="Project"
			hideLabel
			name="repo"
			icon={FolderGit2}
			anyLabel="Every project"
			value={filters.repo ?? ''}
			options={choices('repo')}
			onchange={submit}
		/>
		<Select
			label="Model"
			hideLabel
			name="model"
			icon={Bot}
			anyLabel="Every model"
			value={filters.model ?? ''}
			options={choices('model')}
			onchange={submit}
		/>
	</div>
	<noscript><button type="submit">Apply</button></noscript>
</form>

<style>
	.bar {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2) var(--space-4);
	}

	.selects {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
		min-width: 0;
	}
</style>
