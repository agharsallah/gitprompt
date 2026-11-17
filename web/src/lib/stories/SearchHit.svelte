<script lang="ts">
	import { GitBranch } from '@lucide/svelte';
	import type { SearchHit } from '#lib/api/types.ts';
	import ConnectorBadge from '#lib/prompts/ConnectorBadge.svelte';
	import { factsView } from '#lib/prompts/facts.ts';
	import OriginBadge from '#lib/prompts/OriginBadge.svelte';
	import { isSystem } from '#lib/prompts/origin.ts';
	import { shortDate, timeOfDay } from '#lib/shared/format.ts';
	import { projectHref, promptHref, sessionHref } from '#lib/shared/links.ts';
	import { track } from '#lib/shared/usage.ts';
	import CopyButton from '#lib/ui/CopyButton.svelte';
	import LinkArrow from '#lib/ui/LinkArrow.svelte';
	import RailNode from '#lib/ui/RailNode.svelte';
	import OutcomeLabel from '#lib/stories/OutcomeLabel.svelte';
	import Snippet from '#lib/stories/Snippet.svelte';
	import { anchor, railOutcome } from '#lib/stories/story.ts';

	/**
	 * One search result: where and when, the matching words in context, and doors to its stories.
	 * `href` opens the prompt (default: the Prompts page with it selected), `copy` is its full text.
	 */
	let {
		hit,
		href,
		selected = false,
		copy
	}: { hit: SearchHit; href?: string; selected?: boolean; copy?: string } = $props();

	const h = $derived(hit);
	const system = $derived(isSystem(h));
	const view = $derived(factsView(h.facts));
</script>

<li class={['hit', system && 'system', selected && 'selected']}>
	<span class="node"><RailNode outcome={railOutcome(h.outcome)} /></span>
	<div class="body">
		<p class="meta">
			<time datetime={h.received_at}>{shortDate(h.received_at)}, {timeOfDay(h.received_at)}</time>
			{#if h.facts}<ConnectorBadge connector={h.facts.connector} surface={h.facts.surface} />{/if}
			{#if h.project}<span class="project">{h.project.name}</span>{/if}
			{#if h.branch}<span class="branch"><GitBranch size={12} aria-hidden="true" />{h.branch}</span
				>{/if}
			<OriginBadge prompt={h} />
			<OutcomeLabel outcome={h.outcome} />
			{#if view.model}<span class="model" title={view.model.hint}>{view.model.name}</span>{/if}
			{#if view.cost}<span>{view.cost}</span>{/if}
		</p>
		<a
			class="text"
			href={href ?? promptHref(h.id)}
			title="Prompt"
			aria-current={selected ? 'true' : undefined}
			data-sveltekit-reset="false"><Snippet segments={h.snippet} /></a
		>
		<p class="links">
			{#if h.session}<LinkArrow
					href={sessionHref(h.session, { anchor: anchor(h.id) })}
					title={h.title ? `Session › ${h.title}` : 'Session'}>Show in session story</LinkArrow
				>{/if}
			{#if h.project && !system}<LinkArrow
					href={projectHref(h.project.id, { anchor: anchor(h.id) })}
					title="Project › {h.project.name}">Show in project story</LinkArrow
				>{/if}
		</p>
	</div>
	{#if copy}
		<span class="copy">
			<CopyButton text={copy} label="Copy prompt" ghost say oncopy={() => track('copy.prompt')} />
		</span>
	{/if}
</li>

<style>
	.hit {
		position: relative;
		display: grid;
		grid-template-columns: 1.5rem minmax(0, 1fr);
		gap: var(--space-2);
		padding: var(--space-3) var(--space-3) var(--space-3) var(--space-2);
		border-bottom: var(--border-hairline) solid var(--color-border-subtle);
	}

	.hit.selected {
		border-radius: var(--radius-md);
		background: var(--color-surface-selected);
		box-shadow: inset 0 0 0 1px var(--color-accent-border);
	}

	/* Copy shows on hover and on keyboard focus, and always on touch screens (no hover there). */
	.copy {
		position: absolute;
		top: var(--space-2);
		right: var(--space-2);
		opacity: 0;
	}

	.hit:hover .copy,
	.hit:focus-within .copy,
	.copy:has(:global(.saying)) {
		opacity: 1;
	}

	@media (hover: none) {
		.copy {
			opacity: 1;
		}
	}

	.hit:last-child {
		border-bottom: none;
	}

	.node {
		display: flex;
		justify-content: center;
		padding-top: 2px;
	}

	.body {
		display: grid;
		gap: var(--space-1);
		min-width: 0;
	}

	.model {
		color: var(--color-agent-text);
	}

	.meta {
		display: flex;
		padding-right: var(--space-8);
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1) var(--space-3);
		color: var(--color-text-muted);
		font-size: var(--text-xs);
	}

	.project {
		color: var(--color-text-secondary);
		font-weight: var(--weight-medium);
	}

	.branch {
		display: inline-flex;
		align-items: center;
		gap: 3px;
	}

	.text {
		max-width: var(--reading-max);
		color: var(--color-text);
		line-height: var(--leading-snug);
		text-decoration: none;
	}

	.text:hover {
		text-decoration: underline;
	}

	/* Not typed by you: muted, and labelled by OriginBadge (never colour alone). */
	.system .text {
		color: var(--color-text-muted);
		font-size: var(--text-sm);
	}

	.links {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1) var(--space-4);
		font-size: var(--text-xs);
	}
</style>
