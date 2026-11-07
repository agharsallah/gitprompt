<script lang="ts">
	import {
		CircleAlert,
		CircleCheck,
		Info,
		OctagonAlert,
		TriangleAlert,
		type LucideIcon
	} from '@lucide/svelte';
	import type { Snippet } from 'svelte';

	/** A message about the content around it: what happened, and what to do about it. */
	let {
		tone = 'info',
		title,
		icon,
		actions,
		children,
		live = false
	}: {
		tone?: 'neutral' | 'info' | 'success' | 'warning' | 'danger';
		title?: string;
		icon?: LucideIcon;
		actions?: Snippet;
		children?: Snippet;
		/** Announce it to screen readers when it appears (errors after an action). */
		live?: boolean;
	} = $props();

	const ICONS = {
		neutral: CircleAlert,
		info: Info,
		success: CircleCheck,
		warning: TriangleAlert,
		danger: OctagonAlert
	};
	const Icon = $derived(icon ?? ICONS[tone]);
</script>

<div
	class={['callout', `tone-${tone}`]}
	role={live ? (tone === 'danger' ? 'alert' : 'status') : undefined}
>
	<Icon size={16} aria-hidden="true" />
	<div class="content">
		{#if title}<p class="title">{title}</p>{/if}
		{#if children}<div class="body">{@render children()}</div>{/if}
		{#if actions}<div class="actions">{@render actions()}</div>{/if}
	</div>
</div>

<style>
	.callout {
		--fg: var(--color-text-secondary);
		--bg: var(--color-surface-sunken);
		--line: var(--color-border);

		display: flex;
		gap: var(--space-3);
		padding: var(--space-3) var(--space-4);
		border: var(--border-hairline) solid var(--line);
		border-radius: var(--radius-md);
		background: var(--bg);
		font-size: var(--text-sm);
	}

	.callout > :global(svg) {
		margin-top: 2px;
		color: var(--fg);
	}

	.tone-info {
		--fg: var(--color-info-text);
		--bg: var(--color-info-soft);
		--line: var(--color-info-border);
	}

	.tone-success {
		--fg: var(--color-success-text);
		--bg: var(--color-success-soft);
		--line: var(--color-success-border);
	}

	.tone-warning {
		--fg: var(--color-warning-text);
		--bg: var(--color-warning-soft);
		--line: var(--color-warning-border);
	}

	.tone-danger {
		--fg: var(--color-danger-text);
		--bg: var(--color-danger-soft);
		--line: var(--color-danger-border);
	}

	.content {
		display: grid;
		flex: 1;
		gap: var(--space-1);
		min-width: 0;
	}

	.title {
		color: var(--fg);
		font-weight: var(--weight-semibold);
	}

	.body {
		color: var(--color-text);
		overflow-wrap: anywhere;
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
		margin-top: var(--space-1);
	}
</style>
