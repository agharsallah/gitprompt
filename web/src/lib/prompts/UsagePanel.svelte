<script lang="ts">
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
	import NotAvailable from '#lib/insights/panels/NotAvailable.svelte';
	import type { Availability } from '#lib/sources/capabilities.ts';

	/**
	 * What the turn cost: tokens by kind, cache reuse, models, and cost and time in the API when the
	 * tool reports them (Codex's model-call hooks report tokens only).
	 * Without usage, `availability` (of the prompt's tool) says which layer would report it, or that
	 * its tool can't.
	 */
	let { prompt, availability }: { prompt: Prompt; availability?: Availability } = $props();

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
	{#if availability}
		<NotAvailable what="tokens and cost" {availability} />
	{/if}
{:else}
	<div class="usage">
		<dl class="figures">
			{#if s.cost_usd}
				<div>
					<dt>Cost</dt>
					<dd>{money(s.cost_usd)}</dd>
				</div>
			{/if}
			<div>
				<dt>Model requests</dt>
				<dd>{integer(s.api_requests)}</dd>
			</div>
			{#if s.api_time_s}
				<div>
					<dt>Time in the API</dt>
					<dd>{duration(s.api_time_s)}</dd>
				</div>
			{/if}
			<div>
				<dt>Tokens</dt>
				<dd>{compact(tokenTotal(tokens))}</dd>
			</div>
			{#if s.reasoning_tokens}
				<div>
					<dt title="Output tokens the model spent reasoning, part of the output">Reasoning</dt>
					<dd>{compact(s.reasoning_tokens)}</dd>
				</div>
			{/if}
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

		{#if s.model_call_status && Object.keys(s.model_call_status).some((k) => k !== 'completed')}
			<p class="note">
				Model calls: {Object.entries(s.model_call_status)
					.map(([status, n]) => `${integer(n)} ${status}`)
					.join(', ')}.
			</p>
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
