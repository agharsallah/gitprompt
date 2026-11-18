<script lang="ts">
	import type { StatsQuery } from '#lib/api/query.ts';
	import type { WorthItem } from '#lib/api/types.ts';
	import HideValues from '#lib/leaks/HideValues.svelte';
	import { actionCount, buttonFor, copyCommand } from '#lib/overview/actions.ts';
	import { track } from '#lib/shared/usage.ts';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';

	/**
	 * One thing worth a look: its sentence and its one button (actions.ts). Hiding values asks
	 * first, in place, and `onchanged` runs once they are hidden (the page reloads its data).
	 */
	let {
		item,
		query = {},
		onchanged
	}: { item: WorthItem; query?: StatsQuery; onchanged?: () => void } = $props();

	const button = $derived(buttonFor(item));
	let asking = $state(false);

	function hidden() {
		asking = false;
		track(actionCount(item.kind));
		onchanged?.();
	}
</script>

<li class="item" data-kind={item.kind}>
	<Callout tone={item.kind === 'possible_leak' ? 'warning' : 'neutral'}>
		{item.text}
		{#snippet actions()}
			{#if button.type === 'copy'}
				<Button
					size="sm"
					title="Copy the prompt, to save as {button.file} (GitPrompt writes no file)"
					onclick={() => copyCommand(item.kind, button)}>{button.label}</Button
				>
			{:else if button.type === 'open'}
				<Button size="sm" href={button.href} onclick={() => track(actionCount(item.kind))}
					>{button.label}</Button
				>
			{:else if button.type === 'hide'}
				{#if !asking}
					<Button size="sm" onclick={() => (asking = true)}>{button.label}</Button>
				{/if}
			{:else}
				<a class="plain" href={button.href}>{button.label}</a>
			{/if}
		{/snippet}
	</Callout>
	{#if asking && button.type === 'hide'}
		<HideValues
			findings={button.findings}
			{query}
			ondone={hidden}
			oncancel={() => (asking = false)}
		/>
	{/if}
</li>

<style>
	.item {
		display: grid;
		gap: var(--space-2);
	}

	.plain {
		color: var(--color-accent-text);
		font-size: var(--text-sm);
	}
</style>
