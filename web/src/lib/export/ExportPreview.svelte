<script lang="ts">
	import { svgDataUrl, type ExportMode, type Preview } from '#lib/export/preview.ts';
	import Callout from '#lib/ui/Callout.svelte';
	import Spinner from '#lib/ui/Spinner.svelte';

	/**
	 * The file as it will be saved: HTML in a sandboxed frame, a chart as its image, any other format
	 * as text. A newer rendering shows a small spinner over the previous one.
	 */
	let {
		mode,
		preview,
		busy,
		error
	}: { mode: ExportMode; preview: Preview | null; busy: boolean; error: string } = $props();
</script>

<section class="preview" aria-label="Preview" aria-busy={busy}>
	{#if error}
		<Callout tone="danger" live>{error}</Callout>
	{:else if !preview}
		<div class="loading"><Spinner /> Rendering…</div>
	{:else if mode === 'html'}
		<iframe title="Preview of the HTML file" sandbox="" srcdoc={preview.text}></iframe>
	{:else if mode === 'chart'}
		<div class="chart"><img src={svgDataUrl(preview.text)} alt="Preview of the chart" /></div>
	{:else}
		<pre class="code">{preview.text}</pre>
	{/if}
	{#if busy && preview}<div class="busy"><Spinner size={14} /></div>{/if}
</section>

<style>
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
</style>
