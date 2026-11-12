<script lang="ts">
	import { Download, X } from '@lucide/svelte';
	import {
		chartFileName,
		PRIVATE_DEFAULTS,
		subject,
		type ExportOptions,
		type ExportTarget
	} from '#lib/export/export.ts';
	import { png, save } from '#lib/export/chart.ts';
	import ExportControls from '#lib/export/ExportControls.svelte';
	import ExportPreview from '#lib/export/ExportPreview.svelte';
	import {
		loadChart,
		loadDocument,
		type ChartInfo,
		type ExportMode,
		type Preview
	} from '#lib/export/preview.ts';
	import { Task } from '#lib/shared/task.svelte.ts';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';

	/**
	 * The export preview: pick a format and what to include, see the file exactly as it will be saved
	 * (the preview is the daemon's rendering of that file), then save it. Every option starts at its
	 * most private choice each time the dialog opens; nothing is remembered.
	 */
	let { target, open = $bindable(false) }: { target: ExportTarget; open?: boolean } = $props();

	let dialog: HTMLDialogElement | undefined = $state();
	let mode = $state<ExportMode>('html');
	let options = $state<ExportOptions>({ ...PRIVATE_DEFAULTS });
	let charts = $state<ChartInfo[]>([]);
	let chart = $state('');
	// The file shown and the mode it was rendered for: until a new mode's file arrives, the previous
	// file stays in its own renderer (never HTML shown as text, or CSV in a frame).
	let preview = $state<Preview | null>(null);
	let previewMode = $state<ExportMode>('html');
	const task = new Task();
	const saving = new Task();

	$effect(() => {
		if (!dialog) return;
		if (open && !dialog.open) {
			options = { ...PRIVATE_DEFAULTS };
			dialog.showModal();
		} else if (!open && dialog.open) dialog.close();
	});

	// Re-render whenever the format, an option or the chart changes; a newer request wins over a
	// slower one.
	let request = 0;
	$effect(() => {
		if (!open) return;
		const mine = ++request;
		const opts = { ...options };
		const current = mode;
		const wanted = chart;
		void task.run(async () => {
			if (current === 'chart') {
				const shown = await loadChart(target, opts, wanted);
				if (mine !== request) return;
				charts = shown.charts;
				chart = shown.chart;
				show(current, shown.svg);
			} else {
				const file = await loadDocument(target, current, opts);
				if (mine === request) show(current, file);
			}
		});
	});

	function show(rendered: ExportMode, file: Preview | null) {
		preview = file;
		previewMode = rendered;
	}

	// The previewed bytes are the file: saving never asks the daemon again.
	function download() {
		if (!preview) return;
		const name = previewMode === 'chart' ? chartFileName(target.kind, chart, 'svg') : preview.name;
		save(new Blob([preview.text], { type: preview.type }), name);
	}

	function downloadPng() {
		const svg = preview;
		if (!svg) return;
		void saving.run(async () =>
			save(await png(svg.text), chartFileName(target.kind, chart, 'png'))
		);
	}

	function close() {
		open = false;
	}
</script>

<dialog
	bind:this={dialog}
	class="export"
	aria-labelledby="export-title"
	onclose={() => (open = false)}
	oncancel={() => (open = false)}
>
	<header>
		<div>
			<h2 id="export-title">Export {subject(target.kind)}</h2>
			<p class="muted">
				A file you save and send yourself. Nothing is uploaded; the preview is the file.
			</p>
		</div>
		<Button variant="ghost" icon={X} label="Close" onclick={close} />
	</header>

	<div class="layout">
		<ExportControls kind={target.kind} {charts} bind:mode bind:options bind:chart />
		<ExportPreview mode={previewMode} {preview} busy={task.busy} error={task.error} />
	</div>

	<footer>
		{#if saving.error}<Callout tone="danger" live>{saving.error}</Callout>{/if}
		<span class="name mono muted truncate" title={preview?.name}>{preview?.name ?? ''}</span>
		<div class="actions">
			<Button variant="secondary" onclick={close}>Close</Button>
			{#if mode === 'chart'}
				<Button
					variant="secondary"
					icon={Download}
					disabled={!preview || task.busy}
					onclick={download}>SVG</Button
				>
				<Button
					variant="primary"
					icon={Download}
					disabled={!preview || task.busy}
					loading={saving.busy}
					onclick={downloadPng}>PNG</Button
				>
			{:else}
				<Button
					variant="primary"
					icon={Download}
					disabled={!preview || task.busy}
					onclick={download}>Save file</Button
				>
			{/if}
		</div>
	</footer>
</dialog>

<style>
	dialog.export {
		width: min(72rem, calc(100% - 2 * var(--space-4)));
		height: min(52rem, calc(100% - 2 * var(--space-4)));
		max-width: none;
		max-height: none;
		margin: auto;
		padding: 0;
		border: var(--border-hairline) solid var(--color-border);
		border-radius: var(--radius-xl);
		background: var(--color-surface-raised);
		color: var(--color-text);
		box-shadow: var(--shadow-overlay);
	}

	dialog.export[open] {
		display: flex;
		flex-direction: column;
	}

	dialog::backdrop {
		background: rgb(0 0 0 / 0.45);
	}

	header,
	footer {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-3);
		padding: var(--space-4) var(--space-5);
	}

	header {
		align-items: flex-start;
		border-bottom: var(--border-hairline) solid var(--color-border);
	}

	h2 {
		font-size: var(--text-lg);
	}

	header p {
		margin-top: var(--space-1);
		font-size: var(--text-sm);
	}

	footer {
		flex-wrap: wrap;
		border-top: var(--border-hairline) solid var(--color-border);
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
		margin-left: auto;
	}

	.name {
		min-width: 0;
		flex: 1 1 12rem;
		font-size: var(--text-xs);
	}

	.layout {
		display: grid;
		flex: 1;
		grid-template-columns: minmax(16rem, 22rem) minmax(0, 1fr);
		min-height: 0;
	}

	@media (max-width: 47.99rem) {
		dialog.export {
			width: 100%;
			height: 100%;
			border-radius: 0;
		}

		header,
		footer {
			padding: var(--space-3) var(--space-4);
		}

		.layout {
			grid-template-columns: minmax(0, 1fr);
			grid-template-rows: auto minmax(24rem, 1fr);
			overflow-y: auto;
		}
	}
</style>
