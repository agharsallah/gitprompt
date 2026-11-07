<script lang="ts">
	import type { Doorway } from '#lib/insights/filters.ts';
	import type { Overview } from '#lib/insights/overview.ts';
	import { plural } from '#lib/shared/format.ts';
	import BarList from '#lib/ui/charts/BarList.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import Panel from '#lib/ui/Panel.svelte';
	import NeedsTelemetry from './NeedsTelemetry.svelte';

	/**
	 * Slash commands you ran. Most never reach the hooks as prompts (only those that expand into one
	 * do), so they come from telemetry, per session; without it, from the prompts that named one.
	 * A command links to its prompts when it has some.
	 */
	let { data, link, connected }: { data: Overview; link: Doorway; connected: boolean } = $props();

	const prompted = $derived(
		new Map(data.commands.groups.filter((g) => g.key).map((g) => [g.key!, g.prompts]))
	);
	const fromTelemetry = $derived(data.telemetry.commands.length > 0);
	const items = $derived(
		fromTelemetry
			? data.telemetry.commands.map((c) => ({
					key: `${c.command} ${c.source}`,
					label: c.command,
					value: c.uses,
					hint: `${c.source ?? 'command'}, in ${plural(c.sessions, 'session')}`,
					mono: true,
					href: prompted.has(c.command) ? link({ command: c.command }) : undefined
				}))
			: [...prompted].map(([command, prompts]) => ({
					key: command,
					label: command,
					value: prompts,
					mono: true,
					href: link({ command })
				}))
	);
</script>

<Panel
	title="Which slash commands do I run?"
	description={fromTelemetry
		? 'From telemetry, in the sessions these prompts belong to.'
		: undefined}
>
	{#if items.length}
		<BarList
			label="Slash commands"
			max={8}
			otherLabel="Other commands"
			{items}
			format={(v) => plural(v, 'use')}
		/>
	{:else if !connected}
		<NeedsTelemetry what="slash commands" {connected} />
	{:else}
		<EmptyState compact title="No slash commands">None were run in these sessions.</EmptyState>
	{/if}
</Panel>
