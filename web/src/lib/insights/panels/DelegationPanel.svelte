<script lang="ts">
	import { Ban, Bot } from '@lucide/svelte';
	import type { Doorway } from '#lib/insights/filters.ts';
	import type { Overview } from '#lib/insights/overview.ts';
	import { querySourceLabel, ratio } from '#lib/insights/series.ts';
	import { mcpParts } from '#lib/insights/tools.ts';
	import { integer, money, percent, plural } from '#lib/shared/format.ts';
	import BarList from '#lib/ui/charts/BarList.svelte';
	import Meter from '#lib/ui/charts/Meter.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import Panel from '#lib/ui/Panel.svelte';
	import Facts from './Facts.svelte';

	/** How much of the work went to subagents and integrations, and what the permission system stopped. */
	let { data, link }: { data: Overview; link: Doorway } = $props();

	const t = $derived(data.activity.totals);
	const share = $derived(ratio(t.subagent_tool_calls, t.tool_calls));
	const subagents = $derived(
		data.querySources.groups.filter((g) => g.detail === 'subagent' && g.key)
	);
	const servers = $derived.by(() => {
		const calls = new Map<string, number>();
		for (const g of data.tools.groups) {
			const server = g.key && mcpParts(g.key)?.server;
			if (server) calls.set(server, (calls.get(server) ?? 0) + g.tool_calls);
		}
		return [...calls].sort((a, b) => b[1] - a[1]);
	});
	const stopped = $derived(t.denials + t.rejections);
</script>

<Panel title="How much work goes to subagents and MCP servers?">
	{#if t.tool_calls || subagents.length}
		<div class="stack">
			{#if share !== null}
				<div class="share">
					<p>
						<span class="num">{percent(share)}</span> of tool calls were made by subagents ({plural(
							t.subagents,
							'subagent'
						)} started)
					</p>
					<Meter
						value={share}
						label="Share of tool calls made by subagents"
						color="var(--chart-6)"
					/>
				</div>
			{/if}
			{#if subagents.length}
				<h3 class="sub">Subagent types, by cost</h3>
				<BarList
					label="Subagent cost by type"
					max={6}
					items={subagents.map((g) => ({
						key: g.key!,
						label: querySourceLabel(g.key),
						value: g.cost_usd,
						icon: Bot,
						hint: `${integer(g.api_requests)} requests in ${plural(g.prompts, 'prompt')}`,
						href: link({ query_source: g.key! })
					}))}
					format={money}
				/>
			{/if}
			{#if servers.length}
				<h3 class="sub">MCP servers, by tool calls</h3>
				<BarList
					label="Tool calls by MCP server"
					max={6}
					items={servers.map(([server, calls]) => ({
						key: server,
						label: server,
						value: calls,
						mono: true
					}))}
					format={(v) => plural(v, 'call')}
				/>
			{/if}
			<Facts
				label="Tool calls stopped"
				facts={[
					{
						label: 'Denied or rejected tool calls',
						value: integer(stopped),
						icon: Ban,
						hint: `${integer(t.denials)} by permission rules, ${integer(t.rejections)} by you (telemetry)`,
						href: stopped ? link({ has: ['denial'] }) : undefined
					}
				]}
			/>
		</div>
	{:else}
		<EmptyState compact title="No agentic work">Turns in this window used no tools.</EmptyState>
	{/if}
</Panel>

<style>
	.stack {
		display: grid;
		gap: var(--space-3);
	}

	.share {
		display: grid;
		gap: var(--space-2);
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}

	.num {
		color: var(--color-text);
		font-weight: var(--weight-semibold);
	}

	.sub {
		margin-top: var(--space-2);
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
		font-weight: var(--weight-medium);
		font-stretch: var(--stretch-normal);
		letter-spacing: var(--tracking-normal);
	}
</style>
