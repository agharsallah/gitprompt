<script lang="ts">
	import KeyValue from '#lib/ui/KeyValue.svelte';
	import { humanize, integer } from '#lib/shared/format.ts';

	/**
	 * Any JSON object as a definition list: numbers aligned, lists as tokens, nested objects
	 * indented, path- and id-like strings in mono. New fields appear without code changes.
	 */
	let {
		data,
		rawKeys = false,
		depth = 0
	}: {
		data: Record<string, unknown>;
		/** Show keys as captured (field names) instead of humanized labels. */
		rawKeys?: boolean;
		depth?: number;
	} = $props();

	const LIST_LIMIT = 24;
	const isPlainObject = (v: unknown): v is Record<string, unknown> =>
		!!v && typeof v === 'object' && !Array.isArray(v);
	const looksLikeCode = (s: string) =>
		/^(\/|~\/|\.{1,2}\/|[A-Z]:\\)/.test(s) || /^[0-9a-f-]{12,}$/i.test(s) || /^https?:\/\//.test(s);
</script>

<dl class={['kv', depth > 0 && 'nested']}>
	{#each Object.entries(data) as [key, value] (key)}
		<dt title={rawKeys ? undefined : key} class={[rawKeys && 'mono']}>{rawKeys ? key : humanize(key)}</dt>
		<dd>
			{#if value === null || value === undefined || value === ''}
				<span class="muted">—</span>
			{:else if typeof value === 'number'}
				<span class="num">{Number.isInteger(value) ? integer(value) : value}</span>
			{:else if typeof value === 'boolean'}
				{value ? 'Yes' : 'No'}
			{:else if typeof value === 'string'}
				<span class={[looksLikeCode(value) && 'mono', 'text']}>{value}</span>
			{:else if Array.isArray(value) && value.every((v) => typeof v !== 'object' || v === null)}
				{#if value.length === 0}
					<span class="muted">none</span>
				{:else}
					<ul class="tokens">
						{#each value.slice(0, LIST_LIMIT) as item, i (i)}
							<li class={[typeof item === 'string' && looksLikeCode(item) && 'mono']}>{String(item)}</li>
						{/each}
						{#if value.length > LIST_LIMIT}<li class="more">{value.length - LIST_LIMIT} more</li>{/if}
					</ul>
				{/if}
			{:else if isPlainObject(value) && depth < 2}
				{#if Object.keys(value).length}
					<KeyValue data={value} rawKeys depth={depth + 1} />
				{:else}
					<span class="muted">none</span>
				{/if}
			{:else}
				<code class="json">{JSON.stringify(value)}</code>
			{/if}
		</dd>
	{/each}
</dl>

<style>
	.kv {
		display: grid;
		grid-template-columns: minmax(8rem, max-content) minmax(0, 1fr);
		gap: var(--space-2) var(--space-6);
		margin: 0;
		font-size: var(--text-sm);
	}

	.nested {
		grid-template-columns: minmax(6rem, max-content) minmax(0, 1fr);
		gap: var(--space-1) var(--space-4);
		padding-left: var(--space-3);
		border-left: var(--border-thick) solid var(--color-border);
	}

	dt {
		color: var(--color-text-secondary);
	}

	dt.mono {
		font-size: var(--text-xs);
		line-height: 1.7;
	}

	dd {
		min-width: 0;
		margin: 0;
		overflow-wrap: anywhere;
	}

	.text {
		white-space: pre-wrap;
	}

	.tokens {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1);
	}

	.tokens li {
		padding: 1px var(--space-1_5);
		border-radius: var(--radius-xs);
		background: var(--color-surface-sunken);
		box-shadow: inset 0 0 0 1px var(--color-border-subtle);
		overflow-wrap: anywhere;
	}

	.tokens .more {
		background: none;
		box-shadow: none;
		color: var(--color-text-muted);
	}

	.json {
		font-size: var(--text-xs);
	}

	@media (max-width: 40rem) {
		.kv {
			grid-template-columns: minmax(0, 1fr);
			gap: 0;
		}

		dd {
			margin-bottom: var(--space-2);
		}
	}
</style>
