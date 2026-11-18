<script lang="ts">
	import { Check, Copy } from '@lucide/svelte';
	import { copyCounted } from '#lib/shared/clipboard.ts';
	import type { UsageAction } from '#lib/shared/usage-actions.ts';

	let {
		text,
		label = 'Copy',
		ghost = false,
		say = false,
		caption,
		mono = false,
		hint,
		count
	}: {
		text: string;
		label?: string;
		/** No frame: for toolbars of icon buttons. */ ghost?: boolean;
		/** Writes "Copied" next to the check for a moment (a row's copy, which has no toast). */
		say?: boolean;
		/** Visible words beside the icon, so the button says what it copies (a pill, sized for a row). */
		caption?: string;
		/** The caption is code (a command): set in the code font. */
		mono?: boolean;
		/** The tooltip, when it should say more than the label (what exactly gets copied). */
		hint?: string;
		/** What a copy is counted as (usage-actions.ts): every copy is, so it is not optional. */
		count: UsageAction;
	} = $props();

	let copied = $state(false);
	let timer: ReturnType<typeof setTimeout>;

	async function copy() {
		// A refusal (access can be denied) shows nothing and counts nothing: the text stays selectable.
		if (!(await copyCounted(text, count))) return;
		copied = true;
		clearTimeout(timer);
		timer = setTimeout(() => (copied = false), 1600);
	}
</script>

<button
	type="button"
	class={[
		'copy',
		ghost && 'ghost',
		say && copied && 'saying',
		caption && 'captioned',
		caption && copied && 'done',
		caption && mono && 'mono'
	]}
	onclick={copy}
	aria-label={copied ? 'Copied' : label}
	title={hint ?? label}
>
	{#if copied}<Check size={caption ? 12 : 14} aria-hidden="true" />{#if say || caption}<span
				class="said"
				aria-hidden="true">Copied</span
			>{/if}{:else}<Copy size={caption ? 12 : 14} aria-hidden="true" />{#if caption}<span
				aria-hidden="true">{caption}</span
			>{/if}{/if}
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

	/* Icon and words in a pill, the height of a row's badges, so it reads as an action among them. */
	.captioned {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
		width: auto;
		height: 1.375rem;
		padding: 0 var(--space-2);
		border-radius: var(--radius-full);
		font-size: var(--text-xs);
		font-weight: var(--weight-medium);
		line-height: 1;
		white-space: nowrap;
	}

	.captioned:hover {
		background: var(--color-surface-hover);
	}

	/* A command as the caption: in code, like the line it copies. */
	.mono {
		font-family: var(--font-mono);
		font-weight: var(--weight-regular);
	}

	.done {
		color: var(--color-success-text);
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
