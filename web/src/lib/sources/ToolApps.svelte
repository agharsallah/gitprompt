<script lang="ts">
	import { Ban } from '@lucide/svelte';
	import { resolve } from '$app/paths';
	import type { Source } from '#lib/api/types.ts';
	import { surfaceIcon } from '#lib/prompts/surface.ts';

	/**
	 * Where a tool's prompts can come from (spec `captures`), each with how it is told apart, and the
	 * apps of the tool GitPrompt can't capture (spec `cannot_capture`), with why. Honest by design:
	 * an app that can't be captured within the tool's documented interfaces is listed, not hidden.
	 */
	let { source }: { source: Source } = $props();

	const captures = $derived(source.captures ?? []);
	const cannot = $derived(Object.entries(source.cannot_capture ?? {}));
</script>

{#if captures.length || cannot.length}
	<section class="apps" aria-label="Apps {source.name} captures">
		<h3>Where your prompts come from</h3>
		<ul>
			{#each captures as app (app.id)}
				{@const Icon = surfaceIcon(app)}
				<li>
					<span class="icon" aria-hidden="true"><Icon size={14} /></span>
					<a
						class="name"
						href="{resolve('/prompts')}?source={encodeURIComponent(
							source.id
						)}&surface={encodeURIComponent(app.id)}">{app.name}</a
					>
					<span class="note">{app.note}</span>
				</li>
			{/each}
			{#each cannot as [app, why] (app)}
				<li class="cannot">
					<span class="icon" aria-hidden="true"><Ban size={14} /></span>
					<span class="name">{app}<span class="sr-only">: not captured</span></span>
					<span class="note"><span class="tag">Not captured.</span> {why}</span>
				</li>
			{/each}
		</ul>
	</section>
{/if}

<style>
	.apps {
		display: grid;
		gap: var(--space-2);
	}

	h3 {
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
		font-weight: var(--weight-semibold);
	}

	ul {
		display: grid;
		gap: var(--space-2);
	}

	li {
		display: grid;
		grid-template-columns: 1.25rem minmax(7rem, max-content) minmax(0, 1fr);
		align-items: baseline;
		gap: var(--space-2);
		font-size: var(--text-sm);
	}

	.icon {
		display: inline-grid;
		align-self: center;
		color: var(--color-text-muted);
	}

	.name {
		font-weight: var(--weight-medium);
	}

	a.name {
		color: var(--color-text);
	}

	.note {
		color: var(--color-text-secondary);
	}

	.cannot .name {
		color: var(--color-text-secondary);
	}

	.tag {
		color: var(--color-text);
		font-weight: var(--weight-medium);
	}

	@media (max-width: 40rem) {
		li {
			grid-template-columns: 1.25rem minmax(0, 1fr);
		}

		.note {
			grid-column: 2;
		}
	}
</style>
