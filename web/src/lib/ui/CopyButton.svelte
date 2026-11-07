<script lang="ts">
	import { Check, Copy } from '@lucide/svelte';

	let {
		text,
		label = 'Copy',
		ghost = false
	}: { text: string; label?: string; /** No frame: for toolbars of icon buttons. */ ghost?: boolean } = $props();

	let copied = $state(false);
	let timer: ReturnType<typeof setTimeout>;

	async function copy() {
		try {
			await navigator.clipboard.writeText(text);
			copied = true;
			clearTimeout(timer);
			timer = setTimeout(() => (copied = false), 1600);
		} catch {
			// Clipboard access can be denied; the text stays selectable.
		}
	}
</script>

<button type="button" class={['copy', ghost && 'ghost']} onclick={copy} aria-label={copied ? 'Copied' : label} title={label}>
	{#if copied}<Check size={14} aria-hidden="true" />{:else}<Copy size={14} aria-hidden="true" />{/if}
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

	.ghost {
		background: none;
		box-shadow: none;
	}

	.ghost:hover {
		background: var(--color-surface-hover);
		box-shadow: none;
	}
</style>
