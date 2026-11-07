<script lang="ts">
	import { resolve } from '$app/paths';
	import { names, updateNote, type Availability } from '#lib/sources/capabilities.ts';
	import EmptyState from '#lib/ui/EmptyState.svelte';

	/**
	 * The empty state of a panel whose numbers only some connectors provide (spec `capabilities`):
	 * says which tool could provide them and how, or that a tool can't, rather than showing zeros.
	 */
	let { what, availability: a }: { what: string; availability: Availability } = $props();

	const What = $derived(what.charAt(0).toUpperCase() + what.slice(1));
</script>

<EmptyState compact title={a.available.length ? `No ${what} yet` : `${What} not available`}>
	{#if a.available.length}
		{names(a.available)} reported no {what} for these prompts yet.
	{:else if a.connectable.length}
		{What} come from
		{#each a.connectable as c, i (c.toolId)}{i ? ' or ' : ''}{c.tool}’s {c.layer.toLowerCase()}{/each}.
		<a href="{resolve('/sources')}#tool-{a.connectable[0].toolId}">Connect it</a> to see them.
	{:else if a.updatable.length}
		{What} need a newer version: {updateNote(a)}
	{:else}
		Not available for {names(a.unavailable) || 'the connected tools'}: {a.unavailable.length > 1
			? 'their connectors don’t'
			: 'its connector doesn’t'} report {what}.
	{/if}
	{#if a.available.length || a.connectable.length}
		{#if a.unavailable.length}Not available for {names(a.unavailable)}.{/if}
		{updateNote(a)}
	{:else if a.updatable.length && a.unavailable.length}
		Not available for {names(a.unavailable)}.
	{/if}
</EmptyState>
