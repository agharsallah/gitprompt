<script lang="ts">
	import { Check } from '@lucide/svelte';
	import { resolve } from '$app/paths';
	import type { Source } from '#lib/api/types.ts';
	import { connectQuery, setupSteps } from '#lib/sources/setup.ts';
	import Button from '#lib/ui/Button.svelte';

	/**
	 * First run: the steps from nothing to a captured prompt, each checked off as it happens. A
	 * connect step opens that connector's consent review directly (one click, then one confirmation).
	 */
	let { sources }: { sources: Source[] } = $props();

	const steps = $derived(setupSteps(sources));
	const next = $derived(steps.findIndex((s) => !s.done && !s.optional));
</script>

<section class="setup" aria-labelledby="setup-title">
	<h2 id="setup-title">Start capturing your prompts</h2>
	<ol>
		{#each steps as step, i (step.id)}
			<li class={[step.done && 'done', i === next && 'next']}>
				<span class="marker" aria-hidden="true">
					{#if step.done}<Check size={14} />{:else}{i + 1}{/if}
				</span>
				<div class="text">
					<p class="title">
						{step.title}
						{#if step.optional}<span class="optional">Optional</span>{/if}
						{#if step.done}<span class="sr-only">(done)</span>{/if}
					</p>
					<p class="body">{step.body}</p>
				</div>
				{#if step.connect && !step.done}
					<Button
						href="{resolve('/sources')}{connectQuery(step.connect)}"
						variant={i === next ? 'primary' : 'secondary'}
						size="sm">{step.action}</Button
					>
				{/if}
			</li>
		{/each}
	</ol>
</section>

<style>
	.setup {
		display: grid;
		gap: var(--space-4);
		padding: var(--space-6);
		border-radius: var(--radius-xl);
		background: var(--color-accent-soft);
		box-shadow: inset 0 0 0 1px var(--color-accent-border);
	}

	ol {
		display: grid;
		gap: var(--space-1);
	}

	li {
		position: relative;
		display: grid;
		grid-template-columns: 1.75rem minmax(0, 1fr) auto;
		align-items: center;
		gap: var(--space-3);
		padding: var(--space-2) 0;
	}

	/* The steps sit on a rail, like everything else that happens in order here. */
	li:not(:last-child)::after {
		content: '';
		position: absolute;
		top: calc(50% + 0.875rem);
		bottom: calc(-50% + 0.875rem);
		left: calc(0.875rem - 1px);
		width: 2px;
		background: var(--color-accent-border);
	}

	.marker {
		display: grid;
		place-items: center;
		width: 1.75rem;
		height: 1.75rem;
		border: 2px solid var(--color-accent-border);
		border-radius: 50%;
		background: var(--color-surface);
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
		font-weight: var(--weight-semibold);
	}

	.next .marker {
		border-color: var(--color-accent);
		color: var(--color-accent-text);
	}

	.done .marker {
		border-color: var(--color-accent);
		background: var(--color-accent);
		color: var(--color-text-on-accent);
	}

	.title {
		font-weight: var(--weight-semibold);
	}

	.optional {
		margin-left: var(--space-1);
		color: var(--color-text-muted);
		font-size: var(--text-xs);
		font-weight: var(--weight-medium);
	}

	.done .title {
		color: var(--color-text-secondary);
	}

	.body {
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}
</style>
