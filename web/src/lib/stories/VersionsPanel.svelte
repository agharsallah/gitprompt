<script lang="ts">
	import { ArrowDown, ArrowUp, Equal } from '@lucide/svelte';
	import { resolve } from '$app/paths';
	import { api } from '#lib/api/client.ts';
	import type { Lineage, Version } from '#lib/api/types.ts';
	import { percent, plural, shortDate, timeOfDay } from '#lib/shared/format.ts';
	import Callout from '#lib/ui/Callout.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import RailNode from '#lib/ui/RailNode.svelte';
	import Spinner from '#lib/ui/Spinner.svelte';
	import OutcomeLabel from '#lib/stories/OutcomeLabel.svelte';
	import WordDiff from '#lib/stories/WordDiff.svelte';
	import { anchor, railOutcome } from '#lib/stories/story.ts';

	/**
	 * The versions of a prompt you typed, oldest first: retries in its session and the same
	 * instruction reused elsewhere, each with how its wording changed from the one before and
	 * whether it ended better.
	 */
	let { id }: { id: number } = $props();

	let lineage = $state.raw<Lineage | null>(null);
	let failure = $state('');
	$effect(() => {
		let current = true;
		lineage = null;
		failure = '';
		api
			.lineage(id)
			.then((l) => current && (lineage = l))
			.catch((e) => current && (failure = e instanceof Error ? e.message : String(e)));
		return () => (current = false);
	});

	const PLACES = {
		self: 'This prompt',
		session: 'Same session',
		project: 'Same project',
		elsewhere: 'Another project'
	};
	const CHANGES = {
		improved: { icon: ArrowUp, label: 'Ended better than the version before' },
		worse: { icon: ArrowDown, label: 'Ended worse than the version before' },
		same: { icon: Equal, label: 'Ended like the version before' }
	};
	const others = $derived(lineage ? lineage.versions.length - 1 : 0);
	const where = (v: Version) =>
		v.place === 'elsewhere' && v.project
			? `${PLACES.elsewhere}: ${v.project.name}`
			: PLACES[v.place];
</script>

{#if failure}
	<Callout tone="danger" title="The versions could not be loaded">{failure}</Callout>
{:else if !lineage}
	<p class="loading"><Spinner size={16} /> Looking for other versions</p>
{:else if lineage.versions.length === 0}
	<EmptyState compact title="Not a prompt you typed">
		Versions are only looked for among the prompts you typed.
	</EmptyState>
{:else if others === 0}
	<EmptyState compact title="No other versions">
		You never sent a prompt worded close to this one (similarity of {percent(lineage.threshold)} or more).
	</EmptyState>
{:else}
	<p class="intro">
		{plural(others, 'other version')} of this prompt, oldest first. Changes are shown against the version
		before.
	</p>
	<ol class="versions">
		{#each lineage.versions as v (v.id)}
			{@const change = v.change ? CHANGES[v.change] : null}
			<li class={['version', v.place === 'self' && 'self']}>
				<span class="rail"><RailNode outcome={railOutcome(v.outcome)} /></span>
				<div class="body">
					<p class="meta">
						<strong>{where(v)}</strong>
						<time datetime={v.received_at}
							>{shortDate(v.received_at)}, {timeOfDay(v.received_at)}</time
						>
						<OutcomeLabel outcome={v.outcome} />
						{#if change}<span class="change"
								><change.icon size={12} aria-hidden="true" />{change.label}</span
							>{/if}
						{#if v.place !== 'self'}<span class="similar"
								>{v.same_text ? 'Same text' : `${percent(v.similarity)} similar`}</span
							>{/if}
					</p>
					{#if v.diff?.some(([op]) => op !== 'equal')}<WordDiff diff={v.diff} />{:else}<p
							class="text"
						>
							{v.text}
						</p>{/if}
					<p class="links">
						{#if v.place !== 'self'}<a href="{resolve('/prompts')}?id={v.id}">Open this version</a
							>{/if}
						{#if v.session}<a href="{resolve('/sessions/[id]', { id: v.session })}#{anchor(v.id)}"
								>Show in its session</a
							>{/if}
					</p>
				</div>
			</li>
		{/each}
	</ol>
{/if}

<style>
	.loading {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		color: var(--color-text-secondary);
	}

	.intro {
		margin-bottom: var(--space-4);
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}

	.versions {
		--story-rail: 1.5rem;
		position: relative;
		display: grid;
		grid-template-columns: minmax(0, 1fr);
	}

	.versions::before {
		content: '';
		position: absolute;
		top: 0.5rem;
		bottom: var(--space-5);
		left: calc(var(--story-rail) / 2 - 1px);
		width: 2px;
		background: var(--color-border-strong);
	}

	.version {
		display: grid;
		grid-template-columns: var(--story-rail) minmax(0, 1fr);
		gap: var(--space-3);
	}

	.rail {
		display: flex;
		justify-content: center;
		padding-top: 2px;
	}

	.rail :global(.node) {
		position: relative;
		z-index: 1;
	}

	.body {
		display: grid;
		gap: var(--space-1_5);
		min-width: 0;
		padding-bottom: var(--space-5);
		font-size: var(--text-sm);
	}

	.self .body {
		padding-left: var(--space-3);
		box-shadow: inset 3px 0 0 var(--color-accent);
	}

	.meta {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1) var(--space-3);
		color: var(--color-text-muted);
		font-size: var(--text-xs);
	}

	.meta strong {
		color: var(--color-text);
		font-weight: var(--weight-semibold);
	}

	.change {
		display: inline-flex;
		align-items: center;
		gap: 3px;
		color: var(--color-text-secondary);
	}

	.text {
		max-width: var(--reading-max);
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}

	.links {
		display: flex;
		gap: var(--space-4);
		font-size: var(--text-xs);
	}
</style>
