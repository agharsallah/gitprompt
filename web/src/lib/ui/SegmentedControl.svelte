<script lang="ts" generics="T extends string">
	import type { LucideIcon } from '@lucide/svelte';

	type Option = {
		value: T;
		label: string;
		icon?: LucideIcon;
		/** Links instead of radios: for choices that live in the URL (time range). */
		href?: string;
	};

	/**
	 * One choice out of a few. Radios when the choice is local state; links (with aria-current)
	 * when it is page state in the URL, so it survives reload and the back button.
	 */
	let {
		options,
		value = $bindable(),
		label,
		iconOnly = false,
		size = 'md'
	}: {
		options: Option[];
		value: T;
		label: string;
		/** Hide option text (it stays the accessible name and tooltip). */
		iconOnly?: boolean;
		size?: 'sm' | 'md';
	} = $props();

	const name = $props.id();
	const links = $derived(options.every((o) => o.href));
</script>

{#if links}
	<nav class={['segmented', `size-${size}`]} aria-label={label}>
		{#each options as o (o.value)}
			<a
				class="segment"
				href={o.href}
				aria-current={o.value === value ? 'page' : undefined}
				aria-label={iconOnly ? o.label : undefined}
				title={iconOnly ? o.label : undefined}
				data-sveltekit-reset="false"
			>
				{#if o.icon}<o.icon size={14} aria-hidden="true" />{/if}
				{#if !iconOnly}{o.label}{/if}
			</a>
		{/each}
	</nav>
{:else}
	<div class={['segmented', `size-${size}`]} role="radiogroup" aria-label={label}>
		{#each options as o (o.value)}
			<label class="segment" title={iconOnly ? o.label : undefined}>
				<input
					type="radio"
					{name}
					value={o.value}
					bind:group={value}
					aria-label={iconOnly ? o.label : undefined}
				/>
				{#if o.icon}<o.icon size={14} aria-hidden="true" />{/if}
				{#if !iconOnly}<span>{o.label}</span>{/if}
			</label>
		{/each}
	</div>
{/if}

<style>
	.segmented {
		display: inline-flex;
		flex: none;
		gap: 2px;
		padding: 2px;
		border-radius: var(--radius-md);
		background: var(--color-surface-sunken);
		box-shadow: inset 0 0 0 1px var(--color-border);
	}

	.segment {
		position: relative;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-1_5);
		min-width: var(--control-sm);
		height: calc(var(--control-md) - 4px);
		padding: 0 var(--space-3);
		border-radius: calc(var(--radius-md) - 2px);
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
		font-weight: var(--weight-medium);
		text-decoration: none;
		white-space: nowrap;
		cursor: pointer;
		transition:
			background-color var(--duration-fast) var(--ease-out),
			color var(--duration-fast) var(--ease-out);
	}

	.size-sm .segment {
		height: calc(var(--control-sm) - 4px);
		padding: 0 var(--space-2);
		font-size: var(--text-xs);
	}

	.segment:hover {
		color: var(--color-text);
	}

	.segment[aria-current='page'],
	.segment:has(input:checked) {
		background: var(--color-surface-raised);
		color: var(--color-text);
		box-shadow:
			0 0 0 1px var(--color-border-strong),
			0 1px 2px light-dark(rgb(21 28 27 / 0.06), rgb(0 0 0 / 0.3));
	}

	.segment:has(input:focus-visible) {
		box-shadow: var(--focus-ring);
	}

	input {
		position: absolute;
		inset: 0;
		margin: 0;
		opacity: 0;
		cursor: pointer;
	}
</style>
