<script lang="ts">
	import { CalendarDays, X } from '@lucide/svelte';
	import { untrack } from 'svelte';
	import type { Recap } from '#lib/api/types.ts';
	import { dismissRecap, firstShowing } from '#lib/overview/recap.ts';
	import { targetHref } from '#lib/shared/links.ts';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';

	/**
	 * The weekly recap on the Overview's first screen (overview/recap.ts): shown the first time the
	 * Overview opens in the week it was sent, with a link to the week and a button to dismiss it.
	 */
	let { recap }: { recap?: Recap | null } = $props();

	// Decided once, when the card is first drawn: a reload of the data does not bring it back.
	const shown = untrack(() => (firstShowing(recap) ? recap : null));
	let dismissed = $state(false);

	function dismiss() {
		if (shown) dismissRecap(shown);
		dismissed = true;
	}
</script>

{#if shown && !dismissed}
	<Callout tone="info" icon={CalendarDays} title={shown.notification.title}>
		{shown.notification.body}
		{#snippet actions()}
			<Button size="sm" href={targetHref(shown.notification.target)}>Open the week</Button>
			<Button
				size="sm"
				variant="ghost"
				icon={X}
				label="Dismiss the weekly recap"
				onclick={dismiss}
			/>
		{/snippet}
	</Callout>
{/if}
