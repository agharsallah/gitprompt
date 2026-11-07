<script lang="ts">
	import { Plug } from '@lucide/svelte';
	import type { Connector, Surface } from '#lib/api/types.ts';
	import { sentBy, surfaceIcon } from '#lib/prompts/surface.ts';
	import Badge from '#lib/ui/Badge.svelte';

	/**
	 * The tool that sent a prompt, and where it was typed: the plug icon (the same as the Tool
	 * filter) and its short name, then, when the tool says, the app's icon and name after a hairline
	 * ("Claude Code | Desktop app"). Neutral ink: tools and apps are told apart by their names, never
	 * by colour (lagoon and iris stay reserved for you and the agent). No surface = the tool doesn't
	 * say, so nothing is shown rather than a guess.
	 */
	let {
		connector,
		surface = null,
		size = 'sm'
	}: { connector: Connector; surface?: Surface | null; size?: 'sm' | 'md' } = $props();

	const Icon = $derived(surface ? surfaceIcon(surface) : null);
</script>

<Badge variant="outline" icon={Plug} {size} title={sentBy(connector, surface)}
	>{connector.label}{#if surface && Icon}<span class="surface"
			><Icon size={size === 'sm' ? 11 : 12} aria-hidden="true" /><span class="sr-only"
				>, typed in</span
			>{surface.name}</span
		>{/if}</Badge
>

<style>
	.surface {
		display: inline-flex;
		align-items: center;
		gap: 3px;
		margin-left: var(--space-1_5);
		padding-left: var(--space-1_5);
		border-left: var(--border-hairline) solid var(--color-border);
		color: var(--color-text-muted);
		vertical-align: top;
	}

	.surface :global(svg) {
		flex: none;
	}
</style>
