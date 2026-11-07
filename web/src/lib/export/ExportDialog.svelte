<script lang="ts">
	import { Download, ShieldCheck, X } from '@lucide/svelte';
	import {
		chartFileName,
		chartsUrl,
		exportUrl,
		FORMATS,
		loosened,
		OPTIONS,
		PRIVATE_DEFAULTS,
		SECRETS_NOTE,
		subject,
		type DocumentFormat,
		type ExportOptions,
		type ExportTarget
	} from '#lib/export/export.ts';
	import { png, save } from '#lib/export/chart.ts';
	import { Task } from '#lib/shared/task.svelte.ts';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import SegmentedControl from '#lib/ui/SegmentedControl.svelte';
	import Select from '#lib/ui/Select.svelte';
	import Spinner from '#lib/ui/Spinner.svelte';

	/**
	 * The export preview: pick a format and what to include, see the file exactly as it will be saved
	 * (the preview is the daemon's rendering of that file), then save it. Every option starts at its
	 * most private choice each time the dialog opens; nothing is remembered.
	 */
	let { target, open = $bindable(false) }: { target: ExportTarget; open?: boolean } = $props();

	type Mode = DocumentFormat | 'chart';
	type Preview = { text: string; name: string; type: string };
	type ChartInfo = { id: string; title: string };

	let dialog: HTMLDialogElement | undefined = $state();
	let mode = $state<Mode>('html');
	let options = $state<ExportOptions>({ ...PRIVATE_DEFAULTS });
	let preview = $state<Preview | null>(null);
	let charts = $state<ChartInfo[]>([]);
	let chart = $state('');
	let imageUrl = $state('');
	const task = new Task();
	const saving = new Task();

	const offered = $derived(OPTIONS.filter((o) => !o.kinds || o.kinds.includes(target.kind)));
	const relaxed = $derived(loosened(options).filter((k) => offered.some((o) => o.key === k)));
	const modes = $derived([
		...FORMATS.map((f) => ({ value: f.value as Mode, label: f.label })),
		{ value: 'chart' as Mode, label: 'One chart' }
	]);
	const hint = $derived(
		mode === 'chart'
			? 'A single chart as SVG, or as a PNG image drawn from it.'
			: FORMATS.find((f) => f.value === mode)?.hint
	);

	$effect(() => {
		if (!dialog) return;
		if (open && !dialog.open) {
			options = { ...PRIVATE_DEFAULTS };
			dialog.showModal();
		} else if (!open && dialog.open) dialog.close();
	});

	/** The file name the daemon gives (Content-Disposition), so preview and download agree. */
	function fileName(response: Response, fallback: string): string {
		const header = response.headers.get('content-disposition') ?? '';
		return header.match(/filename="([^"]+)"/)?.[1] ?? fallback;
	}

	async function fetchText(url: string): Promise<Preview> {
		const response = await fetch(url);
		if (!response.ok) throw new Error(`${response.status}: ${await response.text()}`);
		const type = response.headers.get('content-type') ?? 'text/plain';
		return { text: await response.text(), name: fileName(response, 'gitprompt-export'), type };
	}

	// Re-render whenever the format or an option changes; a newer request wins over a slower one.
	let request = 0;
	$effect(() => {
		if (!open) return;
		const mine = ++request;
		const opts = { ...options };
		const current = mode;
		const wanted = chart;
		task.run(async () => {
			if (current === 'chart') {
				const list: ChartInfo[] = await (await fetch(chartsUrl(target, opts))).json();
				const id = list.some((c) => c.id === wanted) ? wanted : (list[0]?.id ?? '');
				const svg = id ? await fetchText(exportUrl(target, 'svg', opts, { chart: id })) : null;
				if (mine !== request) return;
				charts = list;
				chart = id;
				preview = svg;
				if (imageUrl) URL.revokeObjectURL(imageUrl);
				imageUrl = svg ? URL.createObjectURL(new Blob([svg.text], { type: 'image/svg+xml' })) : '';
			} else {
				const file = await fetchText(exportUrl(target, current, opts));
				if (mine === request) preview = file;
			}
		});
	});

	// The previewed bytes are the file: saving never asks the daemon again.
	function download() {
		if (!preview) return;
		const name = mode === 'chart' ? chartFileName(target.kind, chart, 'svg') : preview.name;
		save(new Blob([preview.text], { type: preview.type }), name);
	}

	function downloadPng() {
		const svg = preview;
		if (!svg) return;
		saving.run(async () => save(await png(svg.text), chartFileName(target.kind, chart, 'png')));
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
		<div class="controls">
			<fieldset>
				<legend>Format</legend>
				<SegmentedControl label="Format" size="sm" options={modes} bind:value={mode} />
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
					This file will include {relaxed
						.map((k) =>
							k === 'prompt_text'
								? 'the text of your prompts'
								: k === 'relative_paths'
									? 'absolute paths'
									: k === 'drop_identity'
										? 'account identity'
										: 'raw payloads'
						)
						.join(', ')}. Check the preview before you send it.
				</Callout>
			{/if}
		</div>

		<section class="preview" aria-label="Preview" aria-busy={task.busy}>
			{#if task.error}
				<Callout tone="danger" live>{task.error}</Callout>
			{:else if !preview}
				<div class="loading"><Spinner /> Rendering…</div>
			{:else if mode === 'html'}
				<iframe title="Preview of the HTML file" sandbox="" srcdoc={preview.text}></iframe>
			{:else if mode === 'chart'}
				<div class="chart">
					{#if imageUrl}<img src={imageUrl} alt="Preview of the chart" />{/if}
				</div>
			{:else}
				<pre class="code">{preview.text}</pre>
			{/if}
			{#if task.busy && preview}<div class="busy"><Spinner size={14} /></div>{/if}
		</section>
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

	.preview {
		position: relative;
		display: flex;
		min-width: 0;
		min-height: 0;
		background: var(--color-surface-sunken);
	}

	iframe,
	.code,
	.chart {
		flex: 1;
		width: 100%;
		min-height: 0;
		margin: 0;
		border: 0;
	}

	iframe {
		background: transparent;
		color-scheme: light dark;
	}

	.code {
		padding: var(--space-4);
		overflow: auto;
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		line-height: 1.55;
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}

	.chart {
		display: grid;
		place-items: center;
		padding: var(--space-5);
		overflow: auto;
	}

	.chart img {
		max-width: 100%;
		height: auto;
		border-radius: var(--radius-md);
	}

	.loading {
		display: flex;
		flex: 1;
		align-items: center;
		justify-content: center;
		gap: var(--space-2);
		color: var(--color-text-muted);
	}

	.busy {
		position: absolute;
		top: var(--space-3);
		right: var(--space-3);
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

		.controls {
			overflow: visible;
			padding: var(--space-4);
			border-right: 0;
			border-bottom: var(--border-hairline) solid var(--color-border);
		}
	}
</style>
