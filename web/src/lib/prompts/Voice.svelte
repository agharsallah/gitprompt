<script lang="ts">
	import type { LucideIcon } from '@lucide/svelte';
	import type { Snippet } from 'svelte';
	import { isLong } from '#lib/prompts/detail.ts';

	/**
	 * One voice of a turn: who speaks (the heading: a dot in the voice's colour, or the icon of what
	 * wrote it), what they said, folded when long, and whatever belongs under it. Your words sit on
	 * a lagoon rule, the agent's on an iris one, and text the tool wrote for you (`system`) on a
	 * quiet one.
	 */
	let {
		id,
		tone,
		text,
		moreLabel,
		speaker,
		icon,
		title,
		children
	}: {
		/** The heading's id, which the section is labelled by. */
		id: string;
		tone: 'you' | 'system' | 'agent';
		text: string;
		/** The unfold button's label, e.g. "Show the whole answer". */
		moreLabel: string;
		/** Who speaks: "You", "Codex", "Task notification". */
		speaker: string;
		/** Drawn instead of the voice's dot, for text the tool wrote. */
		icon?: LucideIcon;
		/** A tooltip on the speaker. */
		title?: string;
		children?: Snippet;
	} = $props();

	let expanded = $state(false);
	const long = $derived(isLong(text));
</script>

<section
	class={['voice', tone === 'agent' ? 'agent' : 'you', tone === 'system' && 'system']}
	aria-labelledby={id}
>
	<h2 {id} class="who" {title}>
		{#if icon}
			{@const Icon = icon}
			<Icon size={14} aria-hidden="true" />
		{:else}
			<span class="avatar" aria-hidden="true"></span>
		{/if}{speaker}
	</h2>
	<p class={['said', long && !expanded && 'folded']}>{text}</p>
	{#if long}
		<button
			class="more"
			type="button"
			aria-expanded={expanded}
			onclick={() => (expanded = !expanded)}
		>
			{expanded ? 'Show less' : moreLabel}
		</button>
	{/if}
	{@render children?.()}
</section>

<style>
	.voice {
		--voice: var(--color-accent);
		--voice-text: var(--color-accent-text);
		display: grid;
		gap: var(--space-2);
		padding-left: var(--space-4);
		border-left: 3px solid var(--voice);
	}

	.agent {
		--voice: var(--color-agent);
		--voice-text: var(--color-agent-text);
	}

	/* Not typed by you: a quiet rule and text, with the origin's icon and label as the speaker. */
	.system {
		--voice: var(--color-border-strong);
	}

	.who {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
		font-stretch: var(--stretch-normal);
		letter-spacing: var(--tracking-normal);
	}

	.avatar {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--voice);
	}

	.said {
		max-width: var(--reading-max);
		font-size: var(--text-lg);
		line-height: var(--leading-relaxed);
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}

	.you .said {
		font-weight: var(--weight-medium);
	}

	.agent .said {
		font-size: var(--text-md);
	}

	.system .said {
		color: var(--color-text-secondary);
		font-size: var(--text-md);
		font-weight: var(--weight-regular);
	}

	.folded {
		max-height: 22rem;
		overflow: hidden;
		mask-image: linear-gradient(to bottom, #000 75%, transparent);
	}

	.more {
		justify-self: start;
		color: var(--voice-text);
		font-size: var(--text-sm);
		font-weight: var(--weight-medium);
	}

	.more:hover {
		text-decoration: underline;
	}
</style>
