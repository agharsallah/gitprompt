<script lang="ts">
	import type { Snippet } from 'svelte';

	/**
	 * A region whose content is on its way. Screen readers hear `label` once; sighted users see the
	 * skeleton in `children`, faded in after a short delay so a fast answer (the daemon is local)
	 * never flashes a placeholder. `waiting`: nothing is fetched yet (a panel below the fold waits to
	 * be scrolled near), so its skeleton holds still instead of shimmering: a shimmer that never ends
	 * repaints every frame for nothing while the page sits idle.
	 */
	let {
		label,
		waiting = false,
		children
	}: { label: string; waiting?: boolean; children: Snippet } = $props();
</script>

<div class="loading" role="status" aria-busy="true">
	<span class="sr-only">{label}</span>
	<div class={['placeholder', { waiting }]} aria-hidden="true">{@render children()}</div>
</div>

<style>
	.placeholder {
		animation: appear var(--duration-slow) var(--ease-out) 180ms both;
	}

	/* Read by Skeleton.svelte; inherited, so every skeleton inside holds still. */
	.waiting {
		--skeleton-play: paused;
	}

	@keyframes appear {
		from {
			opacity: 0;
		}
	}
</style>
