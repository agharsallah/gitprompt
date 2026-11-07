<script lang="ts">
	import type { SpecProblem } from '#lib/api/types.ts';
	import { relativePath } from '#lib/shared/format.ts';
	import Callout from '#lib/ui/Callout.svelte';

	/**
	 * Spec files that can't be used, one message each: which file, its error, and what GitPrompt does
	 * instead. Every other connector keeps working; this is only about the files listed.
	 */
	let { problems }: { problems: SpecProblem[] } = $props();
</script>

{#if problems.length}
	<div class="problems">
		{#each problems as p (p.file.path)}
			<Callout
				tone={p.fallback ? 'warning' : 'danger'}
				title="{p.file.name} can’t be used{p.file.origin === 'bundled' ? ' (bundled)' : ''}"
			>
				<p>
					{#if p.fallback}
						Your override is broken, so GitPrompt uses the bundled spec for this connector until you
						fix or delete it.
					{:else}
						This file is skipped until you fix or delete it. Other connectors are not affected.
					{/if}
				</p>
				<p class="path mono">{relativePath(p.file.path)}</p>
				<pre class="error">{p.error}</pre>
			</Callout>
		{/each}
	</div>
{/if}

<style>
	.problems {
		display: grid;
		gap: var(--space-3);
	}

	p + p,
	.error {
		margin-top: var(--space-1_5);
	}

	.path {
		font-size: var(--text-xs);
		overflow-wrap: anywhere;
	}

	.error {
		max-height: 10rem;
		overflow: auto;
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}
</style>
