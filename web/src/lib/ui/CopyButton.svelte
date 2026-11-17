<script lang="ts">
	import { Check, Copy } from '@lucide/svelte';

	let {
		text,
		label = 'Copy',
		ghost = false,
		say = false,
		oncopy
	}: {
		text: string;
		label?: string;
		/** No frame: for toolbars of icon buttons. */ ghost?: boolean;
		/** Writes "Copied" next to the check for a moment (a row's copy, which has no toast). */
		say?: boolean;
		/** Called once the text is on the clipboard. */
		oncopy?: () => void;
	} = $props();

	let copied = $state(false);
	let timer: ReturnType<typeof setTimeout>;

	async function copy() {
		try {
			await navigator.clipboard.writeText(text);
			copied = true;
			oncopy?.();
			clearTimeout(timer);
			timer = setTimeout(() => (copied = false), 1600);
		} catch {
			// Clipboard access can be denied; the text stays selectable.
		}
	}
</script>

<button
	type="button"
	class={['copy', ghost && 'ghost', say && copied && 'saying']}
	onclick={copy}
	aria-label={copied ? 'Copied' : label}
	title={label}
>
	{#if copied}<Check size={14} aria-hidden="true" />{#if say}<span class="said" aria-hidden="true"
				>Copied</span
			>{/if}{:else}<Copy size={14} aria-hidden="true" />{/if}
</button>
<span class="sr-only" aria-live="polite">{copied ? 'Copied to clipboard' : ''}</span>

<style>
	.copy {
		display: grid;
		place-items: center;
		width: var(--control-sm);
		height: var(--control-sm);
		border-radius: var(--radius-sm);
		background: var(--color-surface-raised);
		box-shadow: inset 0 0 0 1px var(--color-border);
		color: var(--color-text-secondary);
	}

	.copy:hover {
		color: var(--color-text);
		box-shadow: inset 0 0 0 1px var(--color-border-control);
	}

	/* "Copied" beside the check: the button grows to hold it. */
	.saying {
		display: flex;
		align-items: center;
		gap: var(--space-1);
		width: auto;
		padding: 0 var(--space-2);
		color: var(--color-success-text);
		font-size: var(--text-xs);
		font-weight: var(--weight-medium);
	}

	.ghost {
		background: none;
		box-shadow: none;
	}

	.ghost:hover {
		background: var(--color-surface-hover);
		box-shadow: none;
	}
</style>
