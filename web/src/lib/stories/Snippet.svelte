<script lang="ts">
	import type { Segment } from '#lib/api/types.ts';

	/** A search snippet: matches in `<mark>`, which screen readers and forced colours keep. */
	let { segments }: { segments: Segment[] } = $props();
</script>

<span class="snippet"
	>{#each segments as s, i (i)}{#if s.hit}<mark>{s.text}</mark>{:else}{s.text}{/if}{/each}</span
>

<style>
	.snippet {
		overflow-wrap: anywhere;
	}

	mark {
		padding: 0 1px;
		border-radius: 2px;
		background: var(--color-accent-soft);
		color: var(--color-accent-text);
		font-weight: var(--weight-semibold);
		box-shadow: inset 0 -1px 0 var(--color-accent);
	}
</style>
