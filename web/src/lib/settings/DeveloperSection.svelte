<script lang="ts">
	import { RefreshCw, Trash2 } from '@lucide/svelte';
	import { onMount } from 'svelte';
	import type { UsageOut } from '#lib/api/types.ts';
	import { usageApi } from '#lib/api/usage.ts';
	import { usageRows } from '#lib/settings/usage.ts';
	import { integer } from '#lib/shared/format.ts';
	import { Task } from '#lib/shared/task.svelte.ts';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import Panel from '#lib/ui/Panel.svelte';

	const DAYS = 30;
	const task = new Task();
	let out = $state.raw<UsageOut | null>(null);

	const load = () =>
		task.run(async () => {
			out = await usageApi.get(DAYS);
		});
	const clear = () =>
		task.run(async () => {
			await usageApi.clear();
			out = await usageApi.get(DAYS);
		});
	onMount(load);

	const rows = $derived(out ? usageRows(out) : []);
	const none = $derived(!!out && !out.counts.length);
</script>

<Panel
	title="Developer"
	tone="sunken"
	description="How often each part of the dashboard is used, counted on this machine only. The counts are kept in GitPrompt's database, never sent anywhere."
>
	{#snippet actions()}
		<Button size="sm" variant="ghost" icon={RefreshCw} loading={task.busy} onclick={load}
			>Refresh</Button
		>
		<Button size="sm" variant="ghost" icon={Trash2} disabled={task.busy || none} onclick={clear}
			>Clear counts</Button
		>
	{/snippet}

	{#if task.error}<Callout tone="danger" live>{task.error}</Callout>{/if}
	{#if out}
		<table>
			<caption class="sr-only">Uses of each part of the dashboard</caption>
			<thead>
				<tr>
					<th scope="col">Action</th>
					<th scope="col" class="num">Last 7 days</th>
					<th scope="col" class="num">Last {DAYS} days</th>
				</tr>
			</thead>
			<tbody>
				{#each rows as row (row.action)}
					<tr class={[!row.month && 'unused']}>
						<td class="mono">{row.action}</td>
						<td class="num">{integer(row.week)}</td>
						<td class="num">{integer(row.month)}</td>
					</tr>
				{/each}
			</tbody>
		</table>
	{/if}
</Panel>

<style>
	table {
		width: 100%;
		font-size: var(--text-sm);
	}

	th {
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
		font-weight: var(--weight-medium);
		text-align: left;
	}

	th,
	td {
		padding: var(--space-1_5) var(--space-2);
	}

	.num {
		text-align: right;
	}

	.mono {
		font-size: var(--text-xs);
	}

	.unused {
		color: var(--color-text-muted);
	}
</style>
