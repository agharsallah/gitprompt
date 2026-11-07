<script lang="ts">
	import { Check } from '@lucide/svelte';
	import { resolve } from '$app/paths';
	import type { Source } from '#lib/api/types.ts';
	import Button from '#lib/ui/Button.svelte';

	/** First run: the three steps from nothing to a captured prompt, each checked off as it happens. */
	let { sources }: { sources: Source[] } = $props();

	const hooks = $derived(sources.find((s) => s.id === 'claude-code'));
	const telemetry = $derived(sources.find((s) => s.id === 'claude-code-otel'));
	const captured = $derived(sources.some((s) => s.total > 0));

	const steps = $derived([
		{
			title: 'Connect Claude Code',
			body: 'Review the exact change to your settings file, then confirm. A backup is kept.',
			done: !!hooks?.connected,
			action: 'Connect'
		},
		{
			title: 'Add telemetry, if you want cost and tokens',
			body: 'Optional. Applies to Claude Code sessions started afterwards.',
			done: !!telemetry?.connected,
			action: 'Add telemetry'
		},
		{
			title: 'Send a prompt',
			body: 'Type anything in a Claude Code session. It appears here as soon as you send it.',
			done: captured
		}
	]);
	const next = $derived(steps.findIndex((s) => !s.done));
</script>

<section class="setup" aria-labelledby="setup-title">
	<h2 id="setup-title">Start capturing your prompts</h2>
	<ol>
		{#each steps as step, i (step.title)}
			<li class={[step.done && 'done', i === next && 'next']}>
				<span class="marker" aria-hidden="true">
					{#if step.done}<Check size={14} />{:else}{i + 1}{/if}
				</span>
				<div class="text">
					<p class="title">
						{step.title}
						{#if step.done}<span class="sr-only">(done)</span>{/if}
					</p>
					<p class="body">{step.body}</p>
				</div>
				{#if step.action && !step.done}
					<Button
						href={resolve('/sources')}
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

	.done .title {
		color: var(--color-text-secondary);
	}

	.body {
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}
</style>
