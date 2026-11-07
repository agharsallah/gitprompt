<script lang="ts">
	import { CircleSlash } from '@lucide/svelte';
	import { note, type Availability } from '#lib/sources/capabilities.ts';

	/** Under numbers that leave a tool out because it can't provide them: "Not available for X." */
	let { availability }: { availability: Availability } = $props();

	const text = $derived(note(availability));
</script>

{#if text}
	<p class="note"><CircleSlash size={14} aria-hidden="true" />{text}</p>
{/if}

<style>
	.note {
		display: flex;
		align-items: center;
		gap: var(--space-1_5);
		color: var(--color-text-muted);
		font-size: var(--text-xs);
	}

	.note :global(svg) {
		flex: none;
	}
</style>
