<script lang="ts">
	import { page } from '$app/state';
	import SegmentedControl from '#lib/ui/SegmentedControl.svelte';
	import { DEFAULT_RANGE, RANGES, type Range, type RangeId } from '#lib/insights/range.ts';

	/**
	 * The time window of an insight page, as links that set `?range=` (the URL is the state).
	 * `fallback` is the window the page shows without `?range=` (the default_range setting).
	 */
	let { range, fallback = DEFAULT_RANGE }: { range: Range; fallback?: RangeId } = $props();

	function href(id: string) {
		const params = new URLSearchParams(page.url.search);
		if (id === fallback) params.delete('range');
		else params.set('range', id);
		const query = params.toString();
		return `${page.url.pathname}${query ? `?${query}` : ''}`;
	}
</script>

<SegmentedControl
	label="Time range"
	value={range.id}
	options={RANGES.map((r) => ({ value: r.id, label: r.short, href: href(r.id) }))}
/>
