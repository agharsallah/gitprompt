<script lang="ts">
	import { page } from '$app/state';
	import SegmentedControl from '#lib/ui/SegmentedControl.svelte';
	import { RANGES, type Range } from '#lib/insights/range.ts';

	/**
	 * The time window of a filtering page, as links that set `?range=` (the URL is the state).
	 * The other parameters stay, except paging (`limit`, `before`): a new window starts from its first page.
	 */
	let { range }: { range: Range } = $props();

	function href(id: string) {
		const params = new URLSearchParams(page.url.search);
		params.set('range', id);
		params.delete('limit');
		params.delete('before');
		const query = params.toString();
		return `${page.url.pathname}${query ? `?${query}` : ''}`;
	}
</script>

<SegmentedControl
	label="Time range"
	value={range.id}
	options={RANGES.map((r) => ({ value: r.id, label: r.short, href: href(r.id) }))}
/>
