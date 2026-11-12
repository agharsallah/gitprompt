<script lang="ts">
	import { ShieldCheck } from '@lucide/svelte';
	import {
		FORMATS,
		loosened,
		loosenedContent,
		offeredOptions,
		SECRETS_NOTE,
		type ExportKind,
		type ExportOptions
	} from '#lib/export/export.ts';
	import type { ChartInfo, ExportMode } from '#lib/export/preview.ts';
	import Callout from '#lib/ui/Callout.svelte';
	import SegmentedControl from '#lib/ui/SegmentedControl.svelte';
	import Select from '#lib/ui/Select.svelte';

	/**
	 * What to export: the format (or one chart, picked from the export's charts), the options this
	 * kind offers, and a warning naming what the file holds once an option is less private.
	 */
	let {
		kind,
		charts,
		mode = $bindable(),
		options = $bindable(),
		chart = $bindable()
	}: {
		kind: ExportKind;
		charts: ChartInfo[];
		mode: ExportMode;
		options: ExportOptions;
		chart: string;
	} = $props();

	const MODES: { value: ExportMode; label: string }[] = [
		...FORMATS.map((f) => ({ value: f.value, label: f.label })),
		{ value: 'chart', label: 'One chart' }
	];

	const offered = $derived(offeredOptions(kind));
	const relaxed = $derived(loosened(options).filter((k) => offered.some((o) => o.key === k)));
	const hint = $derived(
		mode === 'chart'
			? 'A single chart as SVG, or as a PNG image drawn from it.'
			: FORMATS.find((f) => f.value === mode)?.hint
	);
</script>

<div class="controls">
	<fieldset>
		<legend>Format</legend>
		<SegmentedControl label="Format" size="sm" options={MODES} bind:value={mode} />
		{#if hint}<p class="hint muted">{hint}</p>{/if}
		{#if mode === 'chart' && charts.length}
			<Select
				label="Chart"
				options={charts.map((c) => ({ value: c.id, label: c.title }))}
				bind:value={chart}
			/>
		{/if}
	</fieldset>

	<fieldset>
		<legend>What to include</legend>
		<ul class="options">
			{#each offered as o (o.key)}
				<li>
					<label>
						<input type="checkbox" bind:checked={options[o.key]} />
						<span>
							<span class="label">{o.label}</span>
							<span class="description">{o.description}</span>
						</span>
					</label>
				</li>
			{/each}
			<li class="fixed">
				<ShieldCheck size={16} aria-hidden="true" />
				<span>
					<span class="label">Secrets are always hidden</span>
					<span class="description">{SECRETS_NOTE}</span>
				</span>
			</li>
		</ul>
	</fieldset>

	{#if relaxed.length}
		<Callout tone="warning" title="Less private than the defaults">
			This file will include {loosenedContent(relaxed)}. Check the preview before you send it.
		</Callout>
	{/if}
</div>

<style>
	.controls {
		display: grid;
		align-content: start;
		gap: var(--space-5);
		padding: var(--space-4) var(--space-5);
		overflow-y: auto;
		border-right: var(--border-hairline) solid var(--color-border);
	}

	fieldset {
		display: grid;
		gap: var(--space-2_5);
		min-width: 0;
		margin: 0;
		padding: 0;
		border: 0;
	}

	legend {
		margin-bottom: var(--space-2);
		padding: 0;
		font-size: var(--text-sm);
		font-weight: var(--weight-semibold);
	}

	.hint {
		font-size: var(--text-sm);
	}

	.options {
		display: grid;
		gap: var(--space-3);
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.options label,
	.fixed {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr);
		gap: var(--space-2_5);
		align-items: start;
	}

	.options label {
		cursor: pointer;
	}

	input[type='checkbox'] {
		width: 1rem;
		height: 1rem;
		margin: 0.15rem 0 0;
		accent-color: var(--color-accent);
	}

	.fixed {
		padding-top: var(--space-3);
		border-top: var(--border-hairline) solid var(--color-border-subtle);
		color: var(--color-success-text);
	}

	.label {
		display: block;
		color: var(--color-text);
		font-size: var(--text-md);
		font-weight: var(--weight-medium);
	}

	.description {
		display: block;
		margin-top: var(--space-0_5);
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}

	@media (max-width: 47.99rem) {
		.controls {
			overflow: visible;
			padding: var(--space-4);
			border-right: 0;
			border-bottom: var(--border-hairline) solid var(--color-border);
		}
	}
</style>
