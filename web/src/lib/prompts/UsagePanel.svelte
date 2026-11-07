<script lang="ts">
	import { resolve } from '$app/paths';
	import type { Prompt } from '#lib/api/types.ts';
	import { cacheShare, tokensOf, tokenTotal } from '#lib/insights/aggregate.ts';
	import {
		compact,
		duration,
		integer,
		modelName,
		money,
		percent,
		plural
	} from '#lib/shared/format.ts';
	import BarList from '#lib/ui/charts/BarList.svelte';
	import Meter from '#lib/ui/charts/Meter.svelte';
	import SegmentedBar from '#lib/ui/charts/SegmentedBar.svelte';
	import Callout from '#lib/ui/Callout.svelte';

	/** What the turn cost, from telemetry: tokens by kind, cache reuse, models, time in the API. */
	let { prompt }: { prompt: Prompt } = $props();

	const s = $derived(prompt.summary);
	const tokens = $derived(tokensOf(prompt));
	const cache = $derived(cacheShare(tokens));
	const models = $derived(
		Object.entries(s.models ?? {})
			.sort((a, b) => b[1] - a[1])
			.map(([id, value]) => ({ key: id, label: modelName(id), value, hint: id }))
	);
</script>

{#if !s.api_requests}
	<Callout tone="info" title="No usage for this prompt">
		Model, tokens and cost come from Claude Code's telemetry. Connect
		<a href={resolve('/sources')}>Claude Code telemetry</a> on the Sources page; it applies to sessions
		started afterwards.
	</Callout>
{:else}
	<div class="usage">
		<dl class="figures">
			<div>
				<dt>Cost</dt>
				<dd>{money(s.cost_usd)}</dd>
			</div>
			<div>
				<dt>Model requests</dt>
				<dd>{integer(s.api_requests)}</dd>
			</div>
			<div>
				<dt>Time in the API</dt>
				<dd>{duration(s.api_time_s)}</dd>
			</div>
			<div>
				<dt>Tokens</dt>
				<dd>{compact(tokenTotal(tokens))}</dd>
			</div>
		</dl>

		<section class="block">
			<h3>Where the tokens went</h3>
			<!-- Slot order input, output, cache write, cache read is the palette's validated adjacency. -->
			<SegmentedBar
				variant="tall"
				label="Tokens by kind"
				format={(v) => `${integer(v)} tokens`}
				parts={[
					{ key: 'input', label: 'Input', value: tokens.input, color: 'var(--chart-1)' },
					{ key: 'output', label: 'Output', value: tokens.output, color: 'var(--chart-2)' },
					{ key: 'write', label: 'Cache write', value: tokens.cacheWrite, color: 'var(--chart-3)' },
					{ key: 'read', label: 'Cache read', value: tokens.cacheRead, color: 'var(--chart-4)' }
				]}
			/>
			<table class="tokens">
				<caption class="sr-only">Tokens by kind</caption>
				<tbody>
					<tr><th scope="row">Input</th><td class="num">{integer(tokens.input)}</td></tr>
					<tr><th scope="row">Output</th><td class="num">{integer(tokens.output)}</td></tr>
					<tr><th scope="row">Cache write</th><td class="num">{integer(tokens.cacheWrite)}</td></tr>
					<tr><th scope="row">Cache read</th><td class="num">{integer(tokens.cacheRead)}</td></tr>
				</tbody>
			</table>
		</section>

		{#if cache !== null}
			<section class="block">
				<h3>Served from cache <span class="figure">{percent(cache)}</span></h3>
				<Meter value={cache} label="Prompt tokens served from cache" />
				<p class="note">
					Share of the prompt side (input, cache write and cache read) that was read from the prompt
					cache. Higher is cheaper and faster.
				</p>
			</section>
		{/if}

		{#if models.length}
			<section class="block">
				<h3>Models</h3>
				<BarList
					items={models}
					label="Requests by model"
					format={(v) => plural(v, 'request')}
					color="var(--color-agent)"
				/>
			</section>
		{/if}

		{#if s.api_errors || s.tool_rejections}
			<Callout tone="warning">
				{#if s.api_errors}{plural(s.api_errors, 'API error')}.
				{/if}
				{#if s.tool_rejections}You rejected {plural(s.tool_rejections, 'tool call')}.{/if}
			</Callout>
		{/if}
	</div>
{/if}

<style>
	.usage {
		display: grid;
		gap: var(--space-6);
	}

	.figures {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(8rem, 1fr));
		gap: var(--space-4);
		margin: 0;
	}

	.figures div {
		display: grid;
		gap: var(--space-0_5);
		padding-left: var(--space-3);
		border-left: var(--border-thick) solid var(--color-border);
	}

	dt {
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}

	dd {
		margin: 0;
		font-size: var(--text-xl);
		font-weight: var(--weight-semibold);
		font-stretch: var(--stretch-heading);
	}

	.block {
		display: grid;
		gap: var(--space-3);
	}

	h3 {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		font-size: var(--text-md);
	}

	.figure {
		font-size: var(--text-md);
	}

	.tokens {
		width: 100%;
		max-width: 24rem;
		font-size: var(--text-sm);
	}

	.tokens th {
		padding: var(--space-0_5) 0;
		color: var(--color-text-secondary);
		font-weight: var(--weight-regular);
		text-align: left;
	}

	.tokens td {
		text-align: right;
	}

	.note {
		max-width: var(--reading-max);
		color: var(--color-text-muted);
		font-size: var(--text-xs);
	}
</style>
