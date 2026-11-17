<script lang="ts">
	import { page } from '$app/state';
	import { markSeen, shouldShow } from '#lib/prompts/movedNotice.ts';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';

	/**
	 * The one-time line for an old /search bookmark. Decided when the page opens and counted as seen
	 * at once, so it never comes back; it stays until dismissed for as long as this page is open.
	 */
	const show = shouldShow(page.url.searchParams);
	let dismissed = $state(false);
	if (show) markSeen();
</script>

{#if show && !dismissed}
	<Callout tone="info">
		Search now lives here: type words in the box below, or use Find for everything.
		{#snippet actions()}
			<Button size="sm" onclick={() => (dismissed = true)}>Got it</Button>
		{/snippet}
	</Callout>
{/if}
