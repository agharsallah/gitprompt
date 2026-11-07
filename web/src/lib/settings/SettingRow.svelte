<script lang="ts">
	import { Lock } from '@lucide/svelte';
	import type { Snippet } from 'svelte';
	import type { Setting } from '#lib/api/settings.ts';
	import { origin } from '#lib/settings/describe.ts';
	import Badge from '#lib/ui/Badge.svelte';

	/**
	 * One setting: what it is, its control, and where its value comes from. A setting locked by an
	 * environment variable shows its value as text instead of the control, with the reason.
	 */
	let {
		label,
		description,
		setting,
		file,
		value,
		children
	}: {
		label: string;
		description?: string;
		/** A daemon preference; omit for browser-only settings. */
		setting?: Setting;
		file?: string;
		/** The current value in words, shown when the setting is locked. */
		value?: string;
		children: Snippet;
	} = $props();

	const id = $props.id();
	const from = $derived(setting ? origin(setting, file) : null);
</script>

<div class="row">
	<div class="text">
		<p class="label" id="{id}-label">{label}</p>
		{#if description}<p class="description">{description}</p>{/if}
		{#if from && setting}
			<p class="origin">
				<Badge
					size="sm"
					tone={from.tone}
					icon={setting.locked ? Lock : undefined}
					title={from.detail}>{from.label}</Badge
				>
				{#if setting.locked}<span class="muted">{from.detail}</span>{/if}
			</p>
		{/if}
	</div>
	<div class="control" aria-labelledby="{id}-label" role="group">
		{#if setting?.locked}
			<span class="locked">{value ?? String(setting.value)}</span>
		{:else}
			{@render children()}
		{/if}
	</div>
</div>

<style>
	.row {
		display: grid;
		/* The text keeps a readable width; a long value (a path) wraps in the control column. */
		grid-template-columns: minmax(min(20rem, 100%), 1fr) auto;
		gap: var(--space-3) var(--space-6);
		align-items: start;
		padding: var(--space-4) 0;
		border-top: var(--border-hairline) solid var(--color-border-subtle);
	}

	.row:first-child {
		border-top: 0;
		padding-top: 0;
	}

	.text {
		display: grid;
		gap: var(--space-1);
		max-width: var(--reading-max);
	}

	.label {
		font-weight: var(--weight-semibold);
	}

	.description {
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}

	.origin {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
		font-size: var(--text-xs);
	}

	.control {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: flex-end;
		gap: var(--space-2);
	}

	.locked {
		font-weight: var(--weight-medium);
	}

	@media (max-width: 40rem) {
		.row {
			grid-template-columns: minmax(0, 1fr);
		}

		.control {
			justify-content: flex-start;
		}
	}
</style>
