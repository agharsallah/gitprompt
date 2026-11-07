<script lang="ts">
	import type { Snippet } from 'svelte';

	/**
	 * A region whose content is on its way. Screen readers hear `label` once; sighted users see the
	 * skeleton in `children`, faded in after a short delay so a fast answer (the daemon is local)
	 * never flashes a placeholder.
	 */
	let { label, children }: { label: string; children: Snippet } = $props();
</script>

<div class="loading" role="status" aria-busy="true">
	<span class="sr-only">{label}</span>
	<div class="placeholder" aria-hidden="true">{@render children()}</div>
</div>

<style>
	.placeholder {
		animation: appear var(--duration-slow) var(--ease-out) 180ms both;
	}

	@keyframes appear {
		from {
			opacity: 0;
		}
	}
</style>
