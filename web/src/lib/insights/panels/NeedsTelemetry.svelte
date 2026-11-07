<script lang="ts">
	import { resolve } from '$app/paths';
	import EmptyState from '#lib/ui/EmptyState.svelte';

	/**
	 * The empty state of a panel whose numbers come only from the tool's telemetry (OpenTelemetry):
	 * says so, and how to get them, rather than showing zeros.
	 */
	let { what, connected }: { what: string; connected: boolean } = $props();
</script>

<EmptyState compact title="Needs telemetry">
	{#if connected}
		Telemetry is connected, but reported no {what} for these prompts yet.
	{:else}
		{what.charAt(0).toUpperCase() + what.slice(1)} come from Claude Code's telemetry.
		<a href={resolve('/sources')}>Connect it</a> to see them.
	{/if}
</EmptyState>
