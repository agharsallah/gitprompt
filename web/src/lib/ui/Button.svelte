<script lang="ts">
	import type { LucideIcon } from '@lucide/svelte';
	import type { Snippet } from 'svelte';
	import type { HTMLButtonAttributes } from 'svelte/elements';
	import Spinner from '#lib/ui/Spinner.svelte';

	type Props = Omit<HTMLButtonAttributes, 'children'> & {
		/** primary: the one main action in a view. secondary: everything else. ghost: toolbars. */
		variant?: 'primary' | 'secondary' | 'ghost' | 'danger';
		size?: 'sm' | 'md';
		icon?: LucideIcon;
		/** Without children the button is icon-only: `label` is its accessible name and tooltip. */
		label?: string;
		loading?: boolean;
		/** Makes it a toggle button (aria-pressed). */
		pressed?: boolean;
		/** Renders a link styled as a button. */
		href?: string;
		children?: Snippet;
	};

	let {
		variant = 'secondary',
		size = 'md',
		icon: Icon,
		label,
		loading = false,
		pressed,
		href,
		children,
		type = 'button',
		disabled,
		class: className,
		...rest
	}: Props = $props();

	const iconOnly = $derived(!children);
	const classes = $derived([
		'btn',
		`btn-${variant}`,
		`btn-${size}`,
		iconOnly && 'btn-icon',
		className
	]);
	const iconSize = $derived(size === 'sm' ? 14 : 16);
</script>

{#snippet content()}
	{#if loading}
		<Spinner size={iconSize} />
	{:else if Icon}
		<Icon size={iconSize} aria-hidden="true" />
	{/if}
	{#if children}<span class="text">{@render children()}</span>{/if}
{/snippet}

{#if href}
	<a
		{href}
		class={classes}
		aria-label={iconOnly ? label : undefined}
		title={iconOnly ? label : undefined}
		{...rest as Record<string, unknown>}
	>
		{@render content()}
	</a>
{:else}
	<button
		{type}
		class={classes}
		disabled={disabled || loading}
		aria-busy={loading || undefined}
		aria-pressed={pressed}
		aria-label={iconOnly ? label : undefined}
		title={iconOnly ? label : rest.title}
		{...rest}
	>
		{@render content()}
	</button>
{/if}

<style>
	.btn {
		display: inline-flex;
		flex: none;
		align-items: center;
		justify-content: center;
		gap: var(--space-1_5);
		height: var(--control-md);
		padding: 0 var(--space-3);
		border: var(--border-hairline) solid transparent;
		border-radius: var(--radius-md);
		font-size: var(--text-md);
		font-weight: var(--weight-medium);
		line-height: 1;
		white-space: nowrap;
		text-decoration: none;
		user-select: none;
		transition:
			background-color var(--duration-fast) var(--ease-out),
			border-color var(--duration-fast) var(--ease-out),
			color var(--duration-fast) var(--ease-out);
	}

	.btn-sm {
		height: var(--control-sm);
		padding: 0 var(--space-2);
		gap: var(--space-1);
		border-radius: var(--radius-sm);
		font-size: var(--text-sm);
	}

	.btn-icon {
		width: var(--control-md);
		padding: 0;
	}

	.btn-sm.btn-icon {
		width: var(--control-sm);
	}

	.btn-primary {
		background: var(--color-accent);
		color: var(--color-text-on-accent);
	}

	.btn-primary:hover:not(:disabled) {
		background: var(--color-accent-hover);
	}

	.btn-primary:active:not(:disabled) {
		background: var(--color-accent-active);
	}

	.btn-secondary {
		background: var(--color-surface-raised);
		border-color: var(--color-border-strong);
		color: var(--color-text);
	}

	.btn-secondary:hover:not(:disabled) {
		background: var(--color-surface-hover);
		border-color: var(--color-border-control);
	}

	.btn-ghost {
		color: var(--color-text-secondary);
	}

	.btn-ghost:hover:not(:disabled) {
		background: var(--color-surface-hover);
		color: var(--color-text);
	}

	.btn-danger {
		background: var(--color-danger-soft);
		border-color: var(--color-danger-border);
		color: var(--color-danger-text);
	}

	.btn-danger:hover:not(:disabled) {
		border-color: currentColor;
	}

	.btn[aria-pressed='true'] {
		background: var(--color-accent-soft);
		border-color: var(--color-accent-border);
		color: var(--color-accent-text);
	}

	.btn:disabled {
		opacity: 0.5;
	}

	.btn:focus-visible {
		border-radius: var(--radius-md);
	}

	.text {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
	}
</style>
