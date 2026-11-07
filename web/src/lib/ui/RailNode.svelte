<script lang="ts">
	import type { Outcome } from '#lib/insights/outcome.ts';

	/**
	 * GitPrompt's signature mark: a prompt as a commit node on a session's rail. The fill states the
	 * outcome, and the shape differs too (filled, crossed, ringed, hollow), so it never relies on
	 * color alone. Decorative: the row it sits in names the outcome in text.
	 */
	let { outcome, size = 14 }: { outcome: Outcome; size?: number } = $props();
</script>

<svg class={['node', outcome]} width={size} height={size} viewBox="0 0 14 14" aria-hidden="true">
	{#if outcome === 'done'}
		<circle cx="7" cy="7" r="5" />
	{:else if outcome === 'failed'}
		<circle cx="7" cy="7" r="5" />
		<path d="M5 5l4 4M9 5l-4 4" />
	{:else if outcome === 'running'}
		<circle class="halo" cx="7" cy="7" r="6" />
		<circle cx="7" cy="7" r="3" />
	{:else}
		<circle cx="7" cy="7" r="4.5" />
	{/if}
</svg>

<style>
	.node {
		flex: none;
		overflow: visible;
	}

	circle {
		stroke: var(--rail-surface, var(--color-surface));
		stroke-width: 2;
	}

	.done circle {
		fill: var(--color-accent);
	}

	.failed circle {
		fill: var(--color-danger-solid);
	}

	.failed path {
		stroke: var(--color-text-on-accent);
		stroke-width: 1.6;
		stroke-linecap: round;
	}

	.running circle {
		fill: var(--color-accent);
	}

	.running .halo {
		fill: none;
		stroke: var(--color-accent);
		stroke-width: 1.5;
		transform-origin: center;
		animation: pulse 1.6s var(--ease-out) infinite;
	}

	.open circle {
		fill: var(--rail-surface, var(--color-surface));
		stroke: var(--color-text-muted);
		stroke-width: 1.6;
	}

	@keyframes pulse {
		from {
			opacity: 0.9;
			transform: scale(0.6);
		}
		to {
			opacity: 0;
			transform: scale(1.25);
		}
	}
</style>
