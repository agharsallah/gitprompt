<script lang="ts">
	import { resolve } from '$app/paths';
	import type { Marker } from '#lib/api/types.ts';
	import { timeOfDay } from '#lib/shared/format.ts';
	import Disclosure from '#lib/ui/Disclosure.svelte';
	import { MARKERS, markerSummary } from '#lib/stories/story.ts';

	/**
	 * What happened between two chapters that you did not type: task notifications, agent messages,
	 * subagent briefs, compactions. Folded by default, drawn off the rail as a small square (not a
	 * prompt node), each item with its icon and label, and its text quoted, never as your words.
	 */
	let { markers }: { markers: Marker[] } = $props();

	const kinds = $derived([...new Set(markers.map((m) => m.kind))]);
	const href = (m: Marker) => `${resolve('/prompts')}?id=${m.id}`;
</script>

<li class="group">
	<span class="rail" aria-hidden="true"><span class="tick"></span></span>
	<div class="body">
		<Disclosure>
			{#snippet summary()}
				<span class="summary">
					{#each kinds as kind (kind)}{@const Icon = MARKERS[kind].icon}<Icon
							size={14}
							aria-hidden="true"
						/>{/each}
					<span>{markerSummary(markers)}</span>
				</span>
			{/snippet}
			<ol class="items">
				{#each markers as m, i (`${m.kind}-${m.id}-${i}`)}
					{@const info = MARKERS[m.kind]}
					<li>
						<p class="head">
							<info.icon size={13} aria-hidden="true" />
							<a href={href(m)} title={info.hint}>{info.label}</a>
							{#if m.label}<span class="label">{m.label}</span>{/if}
							{#if m.detail}<span class="detail">{m.detail}</span>{/if}
							<time datetime={m.received_at}>{timeOfDay(m.received_at)}</time>
						</p>
						{#if m.excerpt}
							<blockquote class="excerpt">
								<span class="sr-only">{info.label}, not typed by you: </span>{m.excerpt}
							</blockquote>
						{/if}
					</li>
				{/each}
			</ol>
		</Disclosure>
	</div>
</li>

<style>
	.group {
		display: grid;
		grid-template-columns: var(--story-rail) minmax(0, 1fr);
		gap: var(--space-3);
		margin-top: calc(-1 * var(--space-3));
		padding-bottom: var(--space-4);
	}

	.rail {
		display: flex;
		justify-content: center;
		padding-top: 0.7rem;
	}

	/* Not a prompt: a small hollow square on the rail. */
	.tick {
		position: relative;
		z-index: 1;
		width: 8px;
		height: 8px;
		border-radius: 2px;
		background: var(--color-canvas);
		box-shadow:
			0 0 0 1.5px var(--color-text-muted),
			0 0 0 3.5px var(--color-canvas);
	}

	.body {
		min-width: 0;
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}

	.summary {
		display: inline-flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1_5);
		font-weight: var(--weight-regular);
	}

	.summary :global(svg) {
		color: var(--color-text-muted);
	}

	.items {
		display: grid;
		gap: var(--space-3);
	}

	.head {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1) var(--space-2);
		color: var(--color-text-muted);
		font-size: var(--text-xs);
	}

	.head a {
		color: var(--color-text-secondary);
		font-weight: var(--weight-medium);
	}

	.label {
		color: var(--color-text-secondary);
	}

	.detail {
		padding: 0 var(--space-1_5);
		border-radius: var(--radius-full);
		box-shadow: inset 0 0 0 1px var(--color-border);
	}

	.excerpt {
		max-width: var(--reading-max);
		margin: var(--space-1) 0 0;
		padding: var(--space-2) var(--space-3);
		border-radius: var(--radius-md);
		background: var(--color-surface-sunken);
		color: var(--color-text-muted);
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}
</style>
