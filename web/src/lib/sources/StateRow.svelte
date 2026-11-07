<script lang="ts">
	import type { Source } from '#lib/api/types.ts';
	import { clock } from '#lib/shared/clock.svelte.ts';
	import StateBadge from '#lib/sources/StateBadge.svelte';
	import { layerName, present } from '#lib/sources/state.ts';

	/** One connector in a list: its name, its state and the sentence that explains it. */
	let { source, now }: { source: Source; now?: number } = $props();

	const info = $derived(present(source, now ?? clock.now));
</script>

<div class="row">
	<span class="name">{layerName(source)}</span>
	<StateBadge {info} size="sm" />
	<p class="explanation">{info.explanation}</p>
</div>

<style>
	.row {
		display: grid;
		grid-template-columns: minmax(6rem, max-content) minmax(0, 1fr);
		align-items: center;
		gap: var(--space-1) var(--space-3);
	}

	.row > :global(.badge) {
		justify-self: start;
	}

	.name {
		font-weight: var(--weight-medium);
	}

	.explanation {
		grid-column: 2;
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}

	@media (max-width: 30rem) {
		.row {
			grid-template-columns: minmax(0, 1fr);
		}

		.explanation {
			grid-column: 1;
		}
	}
</style>
