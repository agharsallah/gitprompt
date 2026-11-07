<script lang="ts">
	import { resolve } from '$app/paths';
	import type { Source } from '#lib/api/types.ts';
	import { plural } from '#lib/shared/format.ts';
	import Disclosure from '#lib/ui/Disclosure.svelte';

	/** What a connector collects, straight from its spec: kept, received but dropped, never asked for. */
	let { source }: { source: Source } = $props();

	const dropped = $derived(Object.entries(source.not_stored));
	const skipped = $derived(Object.entries(source.not_collected));
</script>

<Disclosure summary="What this collects">
	{#snippet meta()}{plural(source.collects.length, 'event')}{/snippet}
	<div class="collects">
		<section>
			<h4>Stored</h4>
			<ul class="events">
				{#each source.collects as event (event)}<li><code>{event}</code></li>{/each}
			</ul>
		</section>
		{#if dropped.length}
			<section>
				<h4>Received, not stored</h4>
				<dl>
					{#each dropped as [event, why] (event)}
						<dt><code>{event}</code></dt>
						<dd>{why}</dd>
					{/each}
				</dl>
			</section>
		{/if}
		{#if skipped.length}
			<section>
				<h4>Never collected</h4>
				<dl>
					{#each skipped as [event, why] (event)}
						<dt><code>{event}</code></dt>
						<dd>{why}</dd>
					{/each}
				</dl>
			</section>
		{/if}
		<p class="muted">
			The fields each event carries, documented and seen, are on the <a href={resolve('/fields')}
				>Fields page</a
			>.
		</p>
	</div>
</Disclosure>

<style>
	.collects {
		display: grid;
		gap: var(--space-4);
		padding: var(--space-2) var(--space-2) var(--space-1);
		font-size: var(--text-sm);
	}

	h4 {
		margin-bottom: var(--space-1_5);
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
		font-weight: var(--weight-semibold);
		letter-spacing: 0.02em;
		text-transform: uppercase;
	}

	.events {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1);
	}

	code {
		font-size: var(--text-xs);
		overflow-wrap: anywhere;
	}

	dl {
		display: grid;
		grid-template-columns: minmax(0, max-content) minmax(0, 1fr);
		gap: var(--space-1_5) var(--space-3);
		margin: 0;
	}

	dd {
		margin: 0;
		color: var(--color-text-secondary);
	}

	@media (max-width: 40rem) {
		dl {
			grid-template-columns: minmax(0, 1fr);
		}

		dd {
			margin-bottom: var(--space-1_5);
		}
	}
</style>
