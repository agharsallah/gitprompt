<script lang="ts">
	import type { Doorway } from '#lib/insights/filters.ts';
	import type { Overview } from '#lib/insights/overview.ts';
	import { plural } from '#lib/shared/format.ts';
	import BarList from '#lib/ui/charts/BarList.svelte';
	import { missingNotice } from '#lib/sources/missing.ts';
	import { countPanel } from '#lib/insights/counting.ts';
	import Panel from '#lib/ui/Panel.svelte';
	import { READY, type PanelState } from '#lib/ui/panelState.ts';
	import type { Availability } from '#lib/sources/capabilities.ts';
	import CapabilityNote from './CapabilityNote.svelte';

	/**
	 * Slash commands you ran. Most never reach the hooks as prompts (only those that expand into one
	 * do), so they come from telemetry, per session; without it, from the prompts that named one.
	 * A command links to its prompts when it has some.
	 */
	let { data, link, availability }: { data: Overview; link: Doorway; availability: Availability } =
		$props();

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
	const view = $derived<PanelState>(
		items.length
			? READY
			: !availability.available.length
				? { kind: 'unsupported', ...missingNotice('slash commands', availability) }
				: { kind: 'empty', title: 'No slash commands', message: 'None were run in these sessions.' }
	);
</script>

<Panel
	{@attach countPanel('commands')}
	state={view}
	title="Which slash commands do I run?"
	description={fromTelemetry
		? 'From telemetry, in the sessions these prompts belong to.'
		: undefined}
>
	<BarList
		label="Slash commands"
		max={8}
		otherLabel="Other commands"
		{items}
		format={(v) => plural(v, 'use')}
	/>
	<CapabilityNote {availability} />
</Panel>
