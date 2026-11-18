<script lang="ts">
	import { onMount } from 'svelte';
	import type { StatsQuery } from '#lib/api/query.ts';
	import { hiddenText, hideWithApi, PLACE_WORDS, type HideValues } from '#lib/leaks/hide.svelte.ts';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import Skeleton from '#lib/ui/Skeleton.svelte';
	import { toasts } from '#lib/ui/toast.svelte.ts';

	/**
	 * Asks once before hiding the values of some possible leaks: lists each one (masked, its kind,
	 * where it sits), says it cannot be undone, and on confirm hides them all, toasts "Hidden 2
	 * values" and calls `ondone` with the count (the owner reloads its data). Knows nothing of the
	 * page it is on: the findings and the range come in, the outcome goes out.
	 */
	let {
		findings,
		query = {},
		ondone,
		oncancel,
		flow = hideWithApi(findings, query)
	}: {
		findings: readonly string[];
		/** The filters the findings were found under (the board of that range is read). */
		query?: StatsQuery;
		ondone?: (hidden: number) => void;
		oncancel?: () => void;
		/** The flow to draw; default: over the daemon. Samples and tests pass one already loaded. */
		flow?: HideValues;
	} = $props();

	onMount(() => {
		if (flow.phase === 'loading') void flow.load();
	});

	async function confirm() {
		const hidden = await flow.confirm();
		if (hidden) toasts.show(hiddenText(hidden));
		if (flow.phase === 'done') ondone?.(hidden);
	}

	const many = $derived(flow.found.length > 1);
</script>

{#if flow.phase === 'loading'}
	<div class="loading" role="status" aria-label="Reading the values to hide">
		<Skeleton width="60%" />
		<Skeleton width="40%" />
	</div>
{:else if flow.phase === 'error'}
	<Callout tone="danger" title="Nothing hidden" live>
		{flow.hidden ? `${hiddenText(flow.hidden)}, then: ` : ''}{flow.error}
		{#snippet actions()}
			<Button size="sm" onclick={oncancel}>Close</Button>
		{/snippet}
	</Callout>
{:else if flow.phase !== 'done'}
	<Callout tone="warning" title="Hide {many ? 'these values' : 'this value'}?">
		<ul class="values">
			{#each flow.found as leak (leak.id)}
				<li>
					<code>{leak.masked}</code>
					<span class="where">{leak.kind}, in {PLACE_WORDS[leak.place]}</span>
				</li>
			{/each}
		</ul>
		<p class="warn">
			{many ? 'Each value is' : 'The value is'} replaced in the stored text. This cannot be undone.
		</p>
		{#snippet actions()}
			<Button
				size="sm"
				variant="danger"
				loading={flow.phase === 'saving'}
				disabled={flow.phase === 'saving'}
				onclick={confirm}>Hide {many ? `${flow.found.length} values` : 'it'}</Button
			>
			<Button size="sm" variant="ghost" disabled={flow.phase === 'saving'} onclick={oncancel}
				>Cancel</Button
			>
		{/snippet}
	</Callout>
{/if}

<style>
	.loading {
		display: grid;
		gap: var(--space-2);
		padding: var(--space-2) 0;
	}

	.values {
		display: grid;
		gap: var(--space-1);
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.values li {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: var(--space-1) var(--space-2);
	}

	code {
		font-family: var(--font-mono);
	}

	.where {
		color: var(--color-text-secondary);
	}

	.warn {
		margin-top: var(--space-2);
		color: var(--color-text-secondary);
	}
</style>
