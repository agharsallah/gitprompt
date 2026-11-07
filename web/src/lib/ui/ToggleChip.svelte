<script lang="ts">
	import { Check, type LucideIcon } from '@lucide/svelte';
	import type { Snippet } from 'svelte';
	import type { HTMLInputAttributes } from 'svelte/elements';

	/**
	 * A checkbox that looks like a filter pill. It is a real checkbox, so it works inside a plain GET
	 * form (name/value) and with the keyboard (Space).
	 */
	let {
		checked = $bindable(false),
		icon: Icon,
		count,
		children,
		...rest
	}: Omit<HTMLInputAttributes, 'type' | 'children'> & {
		checked?: boolean;
		icon?: LucideIcon;
		/** Shown after the label, e.g. how many items the filter would keep. */
		count?: number;
		children: Snippet;
	} = $props();
</script>

<label class="toggle">
	<input type="checkbox" bind:checked {...rest} />
	<span class="pill">
		{#if checked}<Check size={13} aria-hidden="true" />{:else if Icon}<Icon size={13} aria-hidden="true" />{/if}
		{@render children()}
		{#if count !== undefined}<span class="count num">{count}</span>{/if}
	</span>
</label>

<style>
	.toggle {
		position: relative;
		display: inline-flex;
	}

	input {
		position: absolute;
		inset: 0;
		opacity: 0;
		margin: 0;
		cursor: pointer;
	}

	.pill {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1_5);
		height: var(--control-sm);
		padding: 0 var(--space-3);
		border: var(--border-hairline) solid var(--color-border-strong);
		border-radius: var(--radius-full);
		background: var(--color-surface-raised);
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
		font-weight: var(--weight-medium);
		white-space: nowrap;
		pointer-events: none;
		transition:
			background-color var(--duration-fast) var(--ease-out),
			border-color var(--duration-fast) var(--ease-out);
	}

	.toggle:hover .pill {
		border-color: var(--color-border-control);
		color: var(--color-text);
	}

	input:checked + .pill {
		background: var(--color-accent-soft);
		border-color: var(--color-accent-border);
		color: var(--color-accent-text);
	}

	input:focus-visible + .pill {
		box-shadow: var(--focus-ring);
	}

	.count {
		color: var(--color-text-muted);
		font-size: var(--text-xs);
	}

	input:checked + .pill .count {
		color: inherit;
	}
</style>
