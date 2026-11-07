<script lang="ts">
	import { RefreshCw, RotateCcw } from '@lucide/svelte';
	import { resolve } from '$app/paths';
	import { describeError } from '#lib/errors/describe.ts';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import CodeBlock from '#lib/ui/CodeBlock.svelte';
	import Disclosure from '#lib/ui/Disclosure.svelte';

	/**
	 * An error, said the same way everywhere (lib/errors/describe.ts). `page` fills the content area
	 * (the error page); `block` sits where the failed part would have been and leaves the rest of the
	 * page alone.
	 */
	let {
		error,
		status,
		what,
		variant = 'block',
		note,
		onretry
	}: {
		error: unknown;
		/** The page status, when the error comes from `page.error`. */
		status?: number;
		/** What failed, for generic titles: "the overview", "this panel". */
		what?: string;
		variant?: 'page' | 'block';
		/** A sentence shown before the hint (e.g. that older data is still shown). */
		note?: string;
		/** Load or render it again. Without it, no "Try again" button. */
		onretry?: () => unknown;
	} = $props();

	const d = $derived(describeError(error, status, what));
	const tone = $derived(
		d.kind === 'not-found' ? 'neutral' : d.kind === 'offline' || note ? 'warning' : 'danger'
	);
	let retrying = $state(false);

	async function retry() {
		retrying = true;
		try {
			await onretry?.();
		} finally {
			retrying = false;
		}
	}
</script>

{#snippet actions()}
	{#if d.reload}
		<Button
			variant={variant === 'page' ? 'primary' : 'secondary'}
			size={variant === 'page' ? 'md' : 'sm'}
			icon={RefreshCw}
			onclick={() => location.reload()}>Reload the page</Button
		>
	{:else if d.retry && onretry}
		<Button
			variant={variant === 'page' ? 'primary' : 'secondary'}
			size={variant === 'page' ? 'md' : 'sm'}
			icon={RotateCcw}
			loading={retrying}
			onclick={retry}>Try again</Button
		>
	{/if}
	{#if variant === 'page'}
		<Button
			variant={d.retry || d.reload ? 'secondary' : 'primary'}
			href={resolve('/')}
			data-sveltekit-reload={d.reload || undefined}>Go to the overview</Button
		>
	{/if}
{/snippet}

{#snippet details()}
	{#if d.detail && d.detail !== d.title}
		<Disclosure summary="Details">
			<CodeBlock code={d.detail} label="Error" maxHeight="12rem" />
		</Disclosure>
	{/if}
{/snippet}

{#if variant === 'page'}
	<div class="page" role="alert">
		{#if d.status}<p class="status num">{d.status}</p>{/if}
		<h1>{d.title}</h1>
		<p class="hint">{note ? `${note} ` : ''}{d.hint}</p>
		<div class="actions">{@render actions()}</div>
		{@render details()}
	</div>
{:else}
	<div class="block">
		<Callout
			{tone}
			title={d.title}
			live
			actions={d.reload || (d.retry && onretry) ? actions : undefined}
		>
			<p>{note ? `${note} ` : ''}{d.hint}</p>
			{@render details()}
		</Callout>
	</div>
{/if}

<style>
	.page {
		display: grid;
		gap: var(--space-4);
		max-width: 40rem;
		padding-top: var(--space-12);
	}

	.status {
		color: var(--color-accent-text);
		font-size: var(--text-5xl);
		font-weight: var(--weight-bold);
		font-stretch: 85%;
		line-height: var(--leading-none);
	}

	.hint {
		max-width: var(--reading-max);
		color: var(--color-text-secondary);
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}

	.block :global(details) {
		margin-top: var(--space-2);
	}
</style>
