<script lang="ts">
	import type { CommitTrace } from '#lib/api/types.ts';
	import { TURN_OUTCOMES } from '#lib/insights/outcome.ts';
	import { commitAnchor, firstLine } from '#lib/projects/commits.ts';
	import { excerpt, shortDate, timeOfDay } from '#lib/shared/format.ts';
	import { promptHref, sessionHref } from '#lib/shared/links.ts';
	import { track } from '#lib/shared/usage.ts';
	import CommitRow from '#lib/stories/CommitRow.svelte';
	import { anchor, railOutcome } from '#lib/stories/story.ts';
	import LinkArrow from '#lib/ui/LinkArrow.svelte';
	import RailNode from '#lib/ui/RailNode.svelte';

	/**
	 * A commit read as cause and effect: the commit on top, the prompt it followed under it with how
	 * that turn ended, and links to the prompt and its session. A commit that followed no prompt is
	 * muted and says "by hand"; one that came after its prompt's turn says "likely". The hash links
	 * to the commit on its forge when the daemon knows it (`trace.url`).
	 */
	let { trace, now }: { trace: CommitTrace; now: Date } = $props();

	const prompt = $derived(trace.prompt);
	const when = (iso: string) => `${shortDate(iso, now)} ${timeOfDay(iso)}`;
</script>

<li id={commitAnchor(trace.commit.hash)} class={['row', !prompt && 'hand']} tabindex="-1">
	<CommitRow commit={trace.commit} href={trace.url} time={when} />
	<span class="open">
		{#if prompt}
			<LinkArrow
				href={promptHref(prompt.id)}
				title="Prompt › {firstLine(prompt.text)}"
				onclick={() => track('commit.open.prompt')}>Prompt</LinkArrow
			>
			{#if trace.session}<LinkArrow
					href={sessionHref(trace.session, { anchor: anchor(prompt.id) })}
					title="Session › This prompt">Session</LinkArrow
				>{/if}
		{:else}
			<span class="tag" title="No prompt came before this commit">by hand</span>
		{/if}
	</span>
	{#if prompt}
		<p class="cause">
			<RailNode outcome={railOutcome(prompt.outcome)} size={12} />
			<span class="sr-only">{TURN_OUTCOMES[prompt.outcome].label}:</span>
			<span class="text">{firstLine(prompt.text)}</span>
			{#if trace.how === 'likely'}<span
					class="likely"
					title="Not certain: this prompt is the most likely owner of the commit">likely</span
				>{/if}
			{#if trace.alternative}{@const other = trace.alternative.prompt}<span class="likely"
					>· or <a href={promptHref(other.id)} title="Prompt › {firstLine(other.text)}"
						>{excerpt(firstLine(other.text), 40)}</a
					></span
				>{/if}
		</p>
	{/if}
</li>

<style>
	.row {
		display: grid;
		grid-template-columns: 4.5rem minmax(0, 1fr) auto auto;
		align-items: baseline;
		gap: var(--space-1) var(--space-3);
		padding: var(--space-2) var(--space-2);
		border-top: var(--border-hairline) solid var(--color-border);
		border-radius: var(--radius-sm);
		font-size: var(--text-sm);
		scroll-margin-top: var(--space-6);
	}

	.row:focus,
	.row:target {
		background: var(--color-surface-sunken);
	}

	.row:focus {
		outline: none;
	}

	.row:focus-visible {
		box-shadow: var(--focus-ring);
	}

	.hand {
		color: var(--color-text-secondary);
	}

	.open {
		display: inline-flex;
		justify-content: flex-end;
		gap: var(--space-3);
		min-width: 9rem;
		font-size: var(--text-xs);
	}

	.tag {
		padding: 0 var(--space-2);
		border-radius: var(--radius-full);
		background: var(--color-surface-sunken);
		color: var(--color-text-muted);
	}

	.cause {
		display: flex;
		grid-column: 2 / -1;
		align-items: center;
		gap: var(--space-1_5);
		min-width: 0;
		margin: 0;
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
	}

	.text {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.likely {
		flex: none;
		color: var(--color-text-muted);
		font-style: italic;
	}

	@media (max-width: 40rem) {
		.row {
			grid-template-columns: auto minmax(0, 1fr);
		}

		.row > :global(.meta),
		.open {
			grid-column: 2;
			justify-content: flex-start;
		}
	}
</style>
