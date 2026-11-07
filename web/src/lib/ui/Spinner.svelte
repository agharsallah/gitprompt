<script lang="ts">
	/** A small busy indicator. Decorative: the control it sits in carries aria-busy. */
	let { size = 14 }: { size?: number } = $props();
</script>

<svg class="spinner" width={size} height={size} viewBox="0 0 16 16" aria-hidden="true">
	<circle cx="8" cy="8" r="6" fill="none" stroke="currentColor" stroke-opacity="0.25" stroke-width="2" />
	<path d="M14 8a6 6 0 0 0-6-6" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
</svg>

<style>
	.spinner {
		animation: spin 0.8s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(1turn);
		}
	}
</style>
