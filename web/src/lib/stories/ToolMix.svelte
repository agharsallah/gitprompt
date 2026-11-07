<script lang="ts">
	import { asKind, KIND_INFO, TOOL_KINDS, type ToolKind } from '#lib/insights/tools.ts';
	import { plural } from '#lib/shared/format.ts';
	import SegmentedBar from '#lib/ui/charts/SegmentedBar.svelte';

	/** A turn's tool calls by kind, as a thin bar (the same colours as everywhere else). */
	let { tools, label }: { tools: Record<string, number>; label: string } = $props();

	const parts = $derived.by(() => {
		const byKind = new Map<ToolKind, number>();
		for (const [k, n] of Object.entries(tools))
			byKind.set(asKind(k), (byKind.get(asKind(k)) ?? 0) + n);
		return TOOL_KINDS.filter((k) => byKind.get(k)).map((k) => ({
			key: k,
			label: KIND_INFO[k].label,
			value: byKind.get(k)!,
			color: KIND_INFO[k].color
		}));
	});
</script>

{#if parts.length}
	<div class="mix"><SegmentedBar {parts} {label} format={(v) => plural(v, 'call')} /></div>
{/if}

<style>
	.mix {
		width: min(14rem, 100%);
	}
</style>
