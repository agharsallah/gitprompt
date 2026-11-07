<script lang="ts">
	import { GitBranch, KeyRound, Star } from '@lucide/svelte';
	import type { Prompt } from '#lib/api/types.ts';
	import { OUTCOMES, type Outcome } from '#lib/insights/outcome.ts';
	import { projectOf } from '#lib/insights/project.ts';
	import { timeOfDay } from '#lib/shared/format.ts';
	import RailNode from '#lib/ui/RailNode.svelte';
	import ConnectorBadge from '#lib/prompts/ConnectorBadge.svelte';
	import OriginBadge from '#lib/prompts/OriginBadge.svelte';
	import { isSystem, typedText } from '#lib/prompts/origin.ts';
	import TurnFacts from '#lib/prompts/TurnFacts.svelte';

	/**
	 * One prompt in a list, on its session's rail. `joinPrev` / `joinNext` draw the rail up to the
	 * row above / down to the row below when they belong to the same session.
	 */
	let {
		prompt,
		outcome,
		href,
		selected = false,
		joinPrev = false,
		joinNext = false,
		showProject = true
	}: {
		prompt: Prompt;
		outcome: Outcome;
		href: string;
		selected?: boolean;
		joinPrev?: boolean;
		joinNext?: boolean;
		showProject?: boolean;
	} = $props();

	const project = $derived(projectOf(prompt));
	const secrets = $derived(prompt.stats.secrets?.length ?? 0);
	const text = $derived(typedText(prompt));
</script>

<a
	class={['row', selected && 'selected', isSystem(prompt) && 'system']}
	{href}
	aria-current={selected ? 'true' : undefined}
	data-sveltekit-reset="false"
>
	<span class={['rail', joinPrev && 'prev', joinNext && 'next']}>
		<RailNode {outcome} />
	</span>
	<span class="body">
		<span class="meta">
			<time datetime={prompt.received_at}>{timeOfDay(prompt.received_at)}</time>
			<ConnectorBadge connector={prompt.facts.connector} />
			{#if showProject}
				<span class="project truncate">{project.name}</span>
			{/if}
			{#if prompt.context.git_branch}
				<span class="branch truncate"
					><GitBranch size={12} aria-hidden="true" />{prompt.context.git_branch}</span
				>
			{/if}
			{#if prompt.summary.turn}<span class="turn">Turn {prompt.summary.turn}</span>{/if}
			<OriginBadge {prompt} />
			<span class="sr-only">{OUTCOMES[outcome].label}</span>
			<span class="flags">
				{#if secrets}
					<span class="secret" title="Possible secret in this prompt">
						<KeyRound size={13} aria-hidden="true" /><span class="sr-only">Possible secret</span>
					</span>
				{/if}
				{#if prompt.annotation.starred}
					<span class="star" title="Starred"
						><Star size={13} aria-hidden="true" /><span class="sr-only">Starred</span></span
					>
				{/if}
			</span>
		</span>
		<span class="text clamp-2">{text ?? 'No prompt text was captured'}</span>
		<span class="foot">
			<TurnFacts {prompt} />
			{#each prompt.annotation.tags as tag (tag)}<span class="tag">#{tag}</span>{/each}
		</span>
	</span>
</a>

<style>
	.row {
		--rail-surface: var(--color-surface);
		display: grid;
		grid-template-columns: 1.5rem minmax(0, 1fr);
		gap: var(--space-2);
		padding: 0 var(--space-3) 0 var(--space-2);
		border-radius: var(--radius-md);
		color: inherit;
		text-decoration: none;
	}

	.row:hover {
		--rail-surface: var(--color-surface-hover);
		background: var(--color-surface-hover);
	}

	.row.selected {
		--rail-surface: var(--color-surface-selected);
		background: var(--color-surface-selected);
		box-shadow: inset 0 0 0 1px var(--color-accent-border);
	}

	.row:focus-visible {
		box-shadow: var(--focus-ring);
		border-radius: var(--radius-md);
	}

	/* The rail: a line through the node, joined to neighbors of the same session. */
	.rail {
		position: relative;
		display: flex;
		justify-content: center;
		padding-top: 0.85rem;
	}

	.rail::before,
	.rail::after {
		content: '';
		position: absolute;
		left: 50%;
		width: 2px;
		margin-left: -1px;
		background: var(--color-border-strong);
	}

	.rail.prev::before {
		top: 0;
		height: 1.3rem;
	}

	.rail.next::after {
		top: 1.3rem;
		bottom: 0;
	}

	.rail :global(.node) {
		position: relative;
		z-index: 1;
	}

	.body {
		display: grid;
		gap: var(--space-1);
		min-width: 0;
		padding: var(--space-2_5) 0;
	}

	.meta {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-width: 0;
		color: var(--color-text-muted);
		font-size: var(--text-xs);
	}

	time {
		flex: none;
		font-variant-numeric: tabular-nums;
	}

	.project {
		min-width: 0;
		color: var(--color-text-secondary);
		font-weight: var(--weight-medium);
	}

	.branch {
		display: inline-flex;
		align-items: center;
		gap: 3px;
		min-width: 0;
		max-width: 10rem;
	}

	.branch :global(svg) {
		flex: none;
	}

	.turn {
		flex: none;
	}

	.flags {
		display: inline-flex;
		gap: var(--space-1);
		margin-left: auto;
	}

	.secret {
		color: var(--color-warning-text);
	}

	.star {
		color: var(--color-star-edge);
	}

	.star :global(svg) {
		fill: var(--color-star);
	}

	.text {
		color: var(--color-text);
		line-height: var(--leading-snug);
		overflow-wrap: anywhere;
	}

	/* Not typed by you: muted, and labelled by OriginBadge (never colour alone). */
	.system .text {
		color: var(--color-text-muted);
		font-size: var(--text-sm);
	}

	.foot {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1) var(--space-3);
	}

	.tag {
		color: var(--color-accent-text);
		font-size: var(--text-xs);
		font-weight: var(--weight-medium);
	}
</style>
