<script lang="ts">
	import { onMount } from 'svelte';
	import Lazy from '#lib/async/Lazy.svelte';
	import { palette } from '#lib/palette/open.svelte.ts';
	import { platform } from '#lib/platform/index.ts';
	import ToastHost from '#lib/ui/ToastHost.svelte';

	/**
	 * The page of the desktop recall window (`/?recall=1`, the root layout renders only this): the
	 * palette in recall mode, open whenever the window is in front. The window outlives its
	 * openings, so each time it is shown the palette opens afresh, and when it is hidden the
	 * palette closes. Nothing here polls or waits: the shell tells the page when the window moves.
	 */
	onMount(() => {
		palette.show();
		return platform.onWindowVisible((visible) => (visible ? palette.show() : palette.hide()));
	});
</script>

<Lazy
	load={() => import('#lib/palette/Palette.svelte')}
	props={{ palette, mode: 'recall' as const }}
	label="Opening recall"
	what="recall"
>
	{#snippet pending()}{/snippet}
</Lazy>

<ToastHost />
