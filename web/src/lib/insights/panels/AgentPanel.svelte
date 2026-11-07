<script lang="ts">
	import type { Doorway } from '#lib/insights/filters.ts';
	import type { Overview } from '#lib/insights/overview.ts';
	import { asKind, KIND_INFO, TOOL_KINDS, toolLabel } from '#lib/insights/tools.ts';
	import { integer, plural } from '#lib/shared/format.ts';
	import BarList from '#lib/ui/charts/BarList.svelte';
	import SegmentedBar from '#lib/ui/charts/SegmentedBar.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import Panel from '#lib/ui/Panel.svelte';

	/** What the agent did for these prompts: tool calls by kind (as the daemon classifies them), then by tool. */
	let { data, link }: { data: Overview; link: Doorway } = $props();

	const t = $derived(data.activity.totals);
	const kinds = $derived(
		TOOL_KINDS.map((k) => ({
			kind: k,
			calls: data.toolKinds.groups
				.filter((g) => asKind(g.key) === k)
				.reduce((n, g) => n + g.tool_calls, 0)
		})).filter((k) => k.calls)
	);
	const tools = $derived(
		[...data.tools.groups].filter((g) => g.key).sort((a, b) => b.tool_calls - a.tool_calls)
	);
</script>

<Panel
	title="What does the agent do?"
	description={t.tool_calls
		? `${plural(t.tool_calls, 'tool call')}${t.tool_failures ? `, ${integer(t.tool_failures)} failed` : ''}.`
		: undefined}
>
	{#if kinds.length}
		<div class="stack">
			<SegmentedBar
				variant="tall"
				label="Tool calls by kind"
				parts={kinds.map((k) => ({
					key: k.kind,
					label: KIND_INFO[k.kind].label,
					value: k.calls,
					color: KIND_INFO[k.kind].color
				}))}
				format={(v) => plural(v, 'call')}
			/>
			<BarList
				label="Most used tools"
				max={7}
				otherLabel="Other tools"
				items={tools.map((g) => {
					const info = KIND_INFO[asKind(g.detail)];
					return {
						key: g.key!,
						label: toolLabel(g.key!),
						value: g.tool_calls,
						color: info.color,
						icon: info.icon,
						hint: `in ${plural(g.prompts, 'prompt')}`,
						mono: g.key!.startsWith('mcp__'),
						href: link({ tool: g.key! })
					};
				})}
			/>
		</div>
	{:else}
		<EmptyState compact title="No tool calls"
			>Turns in this window were answered directly.</EmptyState
		>
	{/if}
</Panel>

<style>
	.stack {
		display: grid;
		gap: var(--space-4);
	}
</style>
