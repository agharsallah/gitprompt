<script lang="ts">
	import { ChevronDown, type LucideIcon } from '@lucide/svelte';
	import type { HTMLSelectAttributes } from 'svelte/elements';

	type Option = { value: string; label: string };

	/**
	 * A native select dressed like TextField: one choice out of many (a project, a model). Native, so
	 * it works in a plain GET form and with the keyboard and screen readers as the platform does.
	 * `value` '' is the "any" choice, labelled `anyLabel`.
	 */
	let {
		label,
		options,
		value = $bindable(''),
		anyLabel,
		icon: Icon,
		hideLabel = false,
		...rest
	}: Omit<HTMLSelectAttributes, 'value'> & {
		label: string;
		options: Option[];
		value?: string;
		/** First option, value ''. */
		anyLabel?: string;
		icon?: LucideIcon;
		hideLabel?: boolean;
	} = $props();

	const id = $props.id();
</script>

<div class="field">
	<label for={id} class={[hideLabel && 'sr-only', 'label']}>{label}</label>
	<div class={['control', value && 'set']}>
		{#if Icon}<Icon size={14} aria-hidden="true" />{/if}
		<select {id} bind:value {...rest}>
			{#if anyLabel !== undefined}<option value="">{anyLabel}</option>{/if}
			{#each options as o (o.value)}<option value={o.value}>{o.label}</option>{/each}
		</select>
		<ChevronDown size={14} aria-hidden="true" />
	</div>
</div>

<style>
	.field {
		display: grid;
		gap: var(--space-1);
		min-width: 0;
	}

	.label {
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
		font-weight: var(--weight-medium);
	}

	.control {
		position: relative;
		display: flex;
		align-items: center;
		gap: var(--space-1_5);
		height: var(--control-sm);
		padding: 0 var(--space-2);
		border: var(--border-hairline) solid var(--color-border-control);
		border-radius: var(--radius-sm);
		background: var(--color-surface-raised);
		color: var(--color-text-muted);
		transition: border-color var(--duration-fast) var(--ease-out);
	}

	/* A chosen value reads as an active filter: accent edge, not colour alone (the text says it). */
	.control.set {
		border-color: var(--color-accent-border);
		color: var(--color-accent-text);
	}

	.control:hover {
		border-color: var(--color-text-muted);
	}

	.control:focus-within {
		border-color: var(--color-accent);
		box-shadow: 0 0 0 3px color-mix(in srgb, var(--color-accent) 20%, transparent);
	}

	select {
		flex: 1;
		min-width: 0;
		max-width: 16rem;
		height: 100%;
		padding: 0 var(--space-5) 0 0;
		margin-right: calc(-1 * var(--space-5));
		border: 0;
		background: none;
		color: var(--color-text);
		font-size: var(--text-sm);
		text-overflow: ellipsis;
		appearance: none;
		cursor: pointer;
		outline: none;
	}

	select:focus-visible {
		box-shadow: none;
	}

	.control > :global(svg:last-child) {
		pointer-events: none;
	}
</style>
