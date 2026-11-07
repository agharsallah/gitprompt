<script lang="ts">
	import { GitBranch } from '@lucide/svelte';
	import type { Phase } from '#lib/api/types.ts';
	import { integer, money, plural } from '#lib/shared/format.ts';
	import Button from '#lib/ui/Button.svelte';
	import OutcomeMix from '#lib/stories/OutcomeMix.svelte';
	import PhaseEntry from '#lib/stories/PhaseEntry.svelte';
	import { diffstatText, phaseReason, span, visible } from '#lib/stories/story.ts';

	/**
	 * One phase of a project: why it began, when, on which branch, what it amounted to, then its
	 * prompts on a rail. Long phases show PAGE prompts at a time, so a big project stays light.
	 */
	let { phase }: { phase: Phase } = $props();

	const PAGE = 40;
	let pages = $state(1);
	const shown = $derived(visible(phase.items, pages, PAGE));
	const left = $derived(phase.items.length - shown.length);
	const changes = $derived(diffstatText(phase.diffstat));
</script>

<section class="phase" id="phase-{phase.index}" aria-labelledby="phase-{phase.index}-title">
	<header>
		<div class="titles">
			<h2 id="phase-{phase.index}-title">
				<span class="index">Phase {phase.index}</span>
				<span class="reason">{phaseReason(phase)}</span>
			</h2>
			<p class="when">
				{span(phase.first_at, phase.last_at)}
				{#each phase.branches as b (b)}<span class="branch"
						><GitBranch size={12} aria-hidden="true" />{b}</span
					>{/each}
			</p>
		</div>
		<dl class="figures">
			<div>
				<dt>Prompts</dt>
				<dd>{integer(phase.prompts)}</dd>
			</div>
			<div>
				<dt>Sessions</dt>
				<dd>{integer(phase.sessions)}</dd>
			</div>
			{#if phase.diffstat}<div>
					<dt>Commits</dt>
					<dd title={changes}>{integer(phase.diffstat.commits)}</dd>
				</div>{/if}
			{#if phase.cost_usd}<div>
					<dt>Cost</dt>
					<dd>{money(phase.cost_usd)}</dd>
				</div>{/if}
		</dl>
		<div class="mix">
			<OutcomeMix outcomes={phase.outcomes} label="How the prompts of phase {phase.index} ended" />
		</div>
	</header>

	<ol class="entries" aria-label="Prompts of phase {phase.index}">
		{#each shown as entry (entry.id)}<PhaseEntry {entry} branch={phase.branches[0]} />{/each}
	</ol>
	{#if left}
		<div class="pager">
			<Button size="sm" onclick={() => pages++}
				>Show {plural(Math.min(left, PAGE), 'more prompt')} of {integer(left)}</Button
			>
		</div>
	{/if}
</section>

<style>
	.phase {
		--story-rail: 1.5rem;
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		gap: var(--space-4);
		scroll-margin-top: var(--space-6);
		/* Off-screen phases skip layout and paint until scrolled to. */
		content-visibility: auto;
		contain-intrinsic-size: auto 40rem;
	}

	header {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		gap: var(--space-3) var(--space-6);
		align-items: end;
		padding-bottom: var(--space-3);
		border-bottom: var(--border-hairline) solid var(--color-border);
	}

	.titles {
		display: grid;
		gap: var(--space-1);
		min-width: 0;
	}

	h2 {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: var(--space-1) var(--space-3);
		font-size: var(--text-xl);
	}

	.reason {
		color: var(--color-text-secondary);
		font-size: var(--text-md);
		font-weight: var(--weight-regular);
		font-stretch: var(--stretch-normal);
		letter-spacing: var(--tracking-normal);
	}

	.when {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1) var(--space-3);
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}

	.branch {
		display: inline-flex;
		align-items: center;
		gap: 3px;
	}

	.figures {
		display: flex;
		gap: var(--space-5);
		margin: 0;
	}

	.figures div {
		display: grid;
	}

	dt {
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
	}

	dd {
		margin: 0;
		font-size: var(--text-lg);
		font-weight: var(--weight-semibold);
		font-variant-numeric: tabular-nums;
	}

	.mix {
		grid-column: 1 / -1;
		max-width: 28rem;
	}

	.entries {
		position: relative;
		display: grid;
		grid-template-columns: minmax(0, 1fr);
	}

	.entries::before {
		content: '';
		position: absolute;
		top: 0.5rem;
		bottom: var(--space-5);
		left: calc(var(--story-rail) / 2 - 1px);
		width: 2px;
		background: var(--color-border-strong);
	}

	.pager {
		padding-left: calc(var(--story-rail) + var(--space-3));
	}

	@media (max-width: 40rem) {
		header {
			grid-template-columns: minmax(0, 1fr);
		}
	}
</style>
