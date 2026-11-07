<script lang="ts">
	import type { LucideIcon } from '@lucide/svelte';
	import type { Snippet } from 'svelte';
	import type { Tone } from '#lib/insights/outcome.ts';

	/**
	 * A short fact or status. Tone carries meaning (see tokens.css), so a badge always has words or
	 * an icon with a label too: never color alone.
	 */
	let {
		tone = 'neutral',
		variant = 'soft',
		size = 'md',
		icon: Icon,
		dot = false,
		title,
		children
	}: {
		tone?: Tone;
		/** soft: tinted fill. outline: hairline only. plain: just colored text and icon. */
		variant?: 'soft' | 'outline' | 'plain';
		size?: 'sm' | 'md';
		icon?: LucideIcon;
		/** A leading status dot instead of an icon. */
		dot?: boolean;
		title?: string;
		children: Snippet;
	} = $props();
</script>

<span class={['badge', `tone-${tone}`, `variant-${variant}`, `size-${size}`]} {title}>
	{#if Icon}<Icon size={size === 'sm' ? 12 : 13} aria-hidden="true" />{:else if dot}<span
			class="dot"
			aria-hidden="true"
		></span>{/if}
	<span class="label">{@render children()}</span>
</span>

<style>
	.badge {
		--fg: var(--color-text-secondary);
		--bg: var(--color-surface-sunken);
		--line: var(--color-border);
		--mark: var(--color-text-muted);

		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
		max-width: 100%;
		height: 1.375rem;
		padding: 0 var(--space-2);
		border: var(--border-hairline) solid transparent;
		border-radius: var(--radius-full);
		color: var(--fg);
		font-size: var(--text-xs);
		font-weight: var(--weight-medium);
		line-height: 1;
		white-space: nowrap;
		vertical-align: middle;
	}

	.size-sm {
		height: 1.125rem;
		padding: 0 var(--space-1_5);
		font-size: var(--text-2xs);
	}

	.label {
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.variant-soft {
		background: var(--bg);
	}

	.variant-outline {
		border-color: var(--line);
	}

	.variant-plain {
		height: auto;
		padding: 0;
	}

	.dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: var(--mark);
	}

	.tone-accent {
		--fg: var(--color-accent-text);
		--bg: var(--color-accent-soft);
		--line: var(--color-accent-border);
		--mark: var(--color-accent);
	}

	.tone-agent {
		--fg: var(--color-agent-text);
		--bg: var(--color-agent-soft);
		--line: var(--color-agent-border);
		--mark: var(--color-agent);
	}

	.tone-success {
		--fg: var(--color-success-text);
		--bg: var(--color-success-soft);
		--line: var(--color-success-border);
		--mark: var(--color-success-solid);
	}

	.tone-warning {
		--fg: var(--color-warning-text);
		--bg: var(--color-warning-soft);
		--line: var(--color-warning-border);
		--mark: var(--color-warning-solid);
	}

	.tone-danger {
		--fg: var(--color-danger-text);
		--bg: var(--color-danger-soft);
		--line: var(--color-danger-border);
		--mark: var(--color-danger-solid);
	}

	.tone-info {
		--fg: var(--color-info-text);
		--bg: var(--color-info-soft);
		--line: var(--color-info-border);
		--mark: var(--color-info-solid);
	}
</style>
