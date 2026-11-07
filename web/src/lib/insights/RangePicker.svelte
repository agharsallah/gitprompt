<script lang="ts">
	import { page } from '$app/state';
	import SegmentedControl from '#lib/ui/SegmentedControl.svelte';
	import { DEFAULT_RANGE, RANGES, type Range } from '#lib/insights/range.ts';

	/** The time window of an insight page, as links that set `?range=` (the URL is the state). */
	let { range }: { range: Range } = $props();

	function href(id: string) {
		const params = new URLSearchParams(page.url.search);
		if (id === DEFAULT_RANGE) params.delete('range');
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
