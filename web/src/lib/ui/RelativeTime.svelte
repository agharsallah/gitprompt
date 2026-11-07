<script lang="ts">
	import { clock } from '#lib/shared/clock.svelte.ts';
	import { relative, when } from '#lib/shared/format.ts';

	/** "5 minutes ago" that stays current; the exact time is in the tooltip and the datetime. */
	let { iso }: { iso: string | null | undefined } = $props();
</script>

{#if iso}
	<time datetime={iso} title={when(iso)}>{relative(iso, clock.now)}</time>
{:else}
	<span>never</span>
{/if}
