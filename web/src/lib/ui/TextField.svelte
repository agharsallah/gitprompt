<script lang="ts">
	import { X, type LucideIcon } from '@lucide/svelte';
	import type { HTMLInputAttributes } from 'svelte/elements';

	/** A labelled text input. The label can be visually hidden, never missing. */
	let {
		label,
		hideLabel = false,
		icon: Icon,
		value = $bindable(''),
		clearable = false,
		size = 'md',
		hint,
		...rest
	}: Omit<HTMLInputAttributes, 'size' | 'value'> & {
		label: string;
		hideLabel?: boolean;
		icon?: LucideIcon;
		value?: string;
		/** Adds a clear button while there is text (search fields). */
		clearable?: boolean;
		size?: 'sm' | 'md';
		hint?: string;
	} = $props();

	let input: HTMLInputElement;
	const id = $props.id();

	function clear() {
		value = '';
		input.focus();
		input.dispatchEvent(new Event('input', { bubbles: true }));
	}
</script>

<div class={['field', `size-${size}`]}>
	<label for={id} class={[hideLabel && 'sr-only', 'label']}>{label}</label>
	<div class="control">
		{#if Icon}<Icon size={size === 'sm' ? 14 : 16} aria-hidden="true" />{/if}
		<input
			{id}
			bind:this={input}
			bind:value
			aria-describedby={hint ? `${id}-hint` : undefined}
			{...rest}
		/>
		{#if clearable && value}
			<button type="button" class="clear" onclick={clear} aria-label="Clear {label.toLowerCase()}">
				<X size={14} aria-hidden="true" />
			</button>
		{/if}
	</div>
	{#if hint}<p id="{id}-hint" class="hint">{hint}</p>{/if}
</div>

<style>
	.field {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		gap: var(--space-1);
		min-width: 0;
	}

	.label {
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
		font-weight: var(--weight-medium);
	}

	.control {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		height: var(--control-md);
		padding: 0 var(--space-3);
		border: var(--border-hairline) solid var(--color-border-control);
		border-radius: var(--radius-md);
		background: var(--color-surface-raised);
		color: var(--color-text-muted);
		transition: border-color var(--duration-fast) var(--ease-out);
	}

	.size-sm .control {
		height: var(--control-sm);
		padding: 0 var(--space-2);
		border-radius: var(--radius-sm);
	}

	.control:hover {
		border-color: var(--color-text-muted);
	}

	.control:focus-within {
		border-color: var(--color-accent);
		box-shadow: 0 0 0 3px color-mix(in srgb, var(--color-accent) 20%, transparent);
	}

	input {
		flex: 1;
		min-width: 0;
		height: 100%;
		padding: 0;
		border: 0;
		background: none;
		color: var(--color-text);
		outline: none;
	}

	.size-sm input {
		font-size: var(--text-sm);
	}

	input::placeholder {
		color: var(--color-text-muted);
	}

	input:focus-visible {
		box-shadow: none;
	}

	/* The native search cancel button duplicates ours. */
	input::-webkit-search-cancel-button {
		display: none;
	}

	.clear {
		display: grid;
		place-items: center;
		width: 1.25rem;
		height: 1.25rem;
		border-radius: 50%;
		color: var(--color-text-muted);
	}

	.clear:hover {
		background: var(--color-surface-hover);
		color: var(--color-text);
	}

	.hint {
		color: var(--color-text-muted);
		font-size: var(--text-xs);
	}
</style>
