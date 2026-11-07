<script lang="ts">
	import type { Change, ConfigAction } from '#lib/api/types.ts';
	import { diffCounts } from '#lib/sources/diff.ts';
	import { plural, relativePath } from '#lib/shared/format.ts';
	import Button from '#lib/ui/Button.svelte';
	import CodeBlock from '#lib/ui/CodeBlock.svelte';

	/**
	 * The consent step: the exact change to the tool's config file, then one confirmation. Takes
	 * focus when it opens, so keyboard and screen-reader users land on what they're agreeing to.
	 */
	let {
		change,
		action,
		update = false,
		busy = false,
		autofocus = true,
		onconfirm,
		oncancel
	}: {
		change: Change;
		action: ConfigAction;
		/** Connecting again over an older connection. */
		update?: boolean;
		busy?: boolean;
		/** Move focus to the review when it opens (off for static samples). */
		autofocus?: boolean;
		onconfirm: () => void;
		oncancel: () => void;
	} = $props();

	const file = $derived(relativePath(change.file));
	const counts = $derived(diffCounts(change.diff));
	const confirmLabel = $derived(
		action === 'disconnect' ? 'Disconnect' : update ? 'Update connection' : 'Connect'
	);
	const id = $props.id();

	const focusOnOpen = (node: HTMLElement) => {
		if (autofocus) node.focus();
	};
</script>

<section class="review" aria-labelledby={id}>
	<h4 {id} tabindex="-1" use:focusOnOpen>
		{#if change.diff}
			Review the change to <span class="mono">{file}</span>
		{:else}
			Nothing to change in <span class="mono">{file}</span>
		{/if}
	</h4>
	{#if change.diff}
		<p class="muted note">
			{plural(counts.added, 'line')} added, {plural(counts.removed, 'line')} removed (marked + and −).
			{action === 'disconnect'
				? 'Only what GitPrompt added is removed.'
				: 'A backup of the file is kept.'}
		</p>
		<CodeBlock code={change.diff} diff label={file} maxHeight="22rem" />
	{/if}
	<div class="actions">
		<Button
			variant={action === 'disconnect' ? 'danger' : 'primary'}
			onclick={onconfirm}
			disabled={!change.diff}
			loading={busy}>{confirmLabel}</Button
		>
		<Button variant="ghost" onclick={oncancel} disabled={busy}>Cancel</Button>
	</div>
</section>

<style>
	.review {
		display: grid;
		gap: var(--space-3);
		min-width: 0;
		padding: var(--space-4);
		border-radius: var(--radius-md);
		background: var(--color-surface-sunken);
	}

	h4 {
		font-size: var(--text-md);
		font-weight: var(--weight-semibold);
		outline-offset: 2px;
	}

	h4 .mono {
		font-size: var(--text-sm);
		font-weight: var(--weight-regular);
		overflow-wrap: anywhere;
	}

	.note {
		font-size: var(--text-xs);
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}
</style>
