<script lang="ts">
	import { GitBranch } from '@lucide/svelte';
	import { resolve } from '$app/paths';
	import type { SessionSummary } from '#lib/insights/aggregate.ts';
	import { outcome } from '#lib/insights/outcome.ts';
	import { clock } from '#lib/shared/clock.svelte.ts';
	import {
		duration,
		excerpt,
		integer,
		modelName,
		money,
		plural,
		timeOfDay
	} from '#lib/shared/format.ts';
	import Badge from '#lib/ui/Badge.svelte';
	import Disclosure from '#lib/ui/Disclosure.svelte';
	import RelativeTime from '#lib/ui/RelativeTime.svelte';
	import PromptRow from '#lib/prompts/PromptRow.svelte';
	import { typedText } from '#lib/prompts/origin.ts';
	import TurnStrip from '#lib/sessions/TurnStrip.svelte';

	/** One conversation: what it was about, where, how long, what it cost, and every turn. */
	let { session }: { session: SessionSummary } = $props();

	const s = $derived(session);
	const span = $derived((Date.parse(s.endedAt) - Date.parse(s.startedAt)) / 1000);
	const title = $derived(
		s.title ?? (excerpt(s.prompts[0] && typedText(s.prompts[0]), 110) || 'Untitled session')
	);
	const lastId = $derived(s.prompts.at(-1)?.id);
	const real = $derived(!s.id.startsWith('prompt-'));
</script>

<article class="session" id="session-{s.id}" aria-labelledby="session-{s.id}-title">
	<header>
		<div class="heading">
			<h2 id="session-{s.id}-title" class="title">
				{#if real}<a href={resolve('/sessions/[id]', { id: s.id })}>{title}</a>{:else}{title}{/if}
			</h2>
			<p class="where">
				<span class="project">{s.project.name}</span>
				{#if s.branch}<span class="branch"
						><GitBranch size={12} aria-hidden="true" />{s.branch}</span
					>{/if}
				<span
					><RelativeTime iso={s.startedAt} />, {timeOfDay(s.startedAt)} to {timeOfDay(
						s.endedAt
					)}</span
				>
			</p>
		</div>
		<div class="badges">
			{#if s.failed}<Badge tone="danger" size="sm">{plural(s.failed, 'failed turn')}</Badge>{/if}
			{#each s.permissionModes as mode (mode)}<Badge variant="outline" size="sm">{mode}</Badge
				>{/each}
		</div>
	</header>

	<div class="body">
		<dl class="figures">
			<div>
				<dt>Turns</dt>
				<dd>{integer(s.prompts.length)}</dd>
			</div>
			<div>
				<dt>Span</dt>
				<dd>{span > 0 ? duration(span) : '—'}</dd>
			</div>
			<div>
				<dt>Tool calls</dt>
				<dd>{integer(s.toolCalls)}</dd>
			</div>
			<div>
				<dt>Files changed</dt>
				<dd>{integer(s.filesWritten)}</dd>
			</div>
			{#if s.cost}<div>
					<dt>Cost</dt>
					<dd>{money(s.cost)}</dd>
				</div>{/if}
		</dl>
		<div class="strip">
			<TurnStrip prompts={s.prompts} />
			{#if s.models.length}
				<p class="models">{s.models.map(modelName).join(', ')}</p>
			{/if}
		</div>
	</div>

	<Disclosure summary="Every turn">
		{#snippet meta()}
			{#if real}<a href={resolve('/sessions/[id]', { id: s.id })}>Read the story</a>
				<a href="{resolve('/prompts')}?session={encodeURIComponent(s.id)}">Open in prompts</a>{/if}
		{/snippet}
		<ol class="turns">
			{#each s.prompts as p, i (p.id)}
				<li>
					<PromptRow
						prompt={p}
						outcome={outcome(p, clock.now, p.id !== lastId)}
						href="{resolve('/prompts')}?id={p.id}"
						joinPrev={i > 0}
						joinNext={i < s.prompts.length - 1}
						showProject={false}
					/>
				</li>
			{/each}
		</ol>
	</Disclosure>
</article>

<style>
	.session {
		display: grid;
		gap: var(--space-4);
		padding: var(--space-5);
		border-radius: var(--radius-lg);
		background: var(--color-surface);
		box-shadow: inset 0 0 0 1px var(--color-border);
		scroll-margin-top: var(--space-6);
	}

	.session:target {
		box-shadow:
			inset 0 0 0 1px var(--color-accent-border),
			0 0 0 3px color-mix(in srgb, var(--color-accent) 20%, transparent);
	}

	header {
		display: flex;
		flex-wrap: wrap;
		align-items: flex-start;
		justify-content: space-between;
		gap: var(--space-2) var(--space-4);
	}

	.heading {
		display: grid;
		gap: var(--space-1);
		min-width: 0;
		flex: 1 1 20rem;
	}

	.title {
		max-width: var(--reading-max);
		font-size: var(--text-lg);
		font-stretch: var(--stretch-normal);
		letter-spacing: var(--tracking-normal);
		line-height: var(--leading-snug);
		overflow-wrap: anywhere;
	}

	.title a {
		color: inherit;
		text-decoration: none;
	}

	.title a:hover {
		color: var(--color-accent-text);
		text-decoration: underline;
	}

	.where {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1) var(--space-3);
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}

	.project {
		color: var(--color-text);
		font-weight: var(--weight-medium);
	}

	.branch {
		display: inline-flex;
		align-items: center;
		gap: 3px;
	}

	.badges {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1);
	}

	.body {
		display: flex;
		flex-wrap: wrap;
		align-items: flex-end;
		justify-content: space-between;
		gap: var(--space-4) var(--space-8);
	}

	.figures {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-3) var(--space-6);
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
	}

	.strip {
		display: grid;
		justify-items: end;
		gap: var(--space-1);
		max-width: 100%;
	}

	.models {
		color: var(--color-agent-text);
		font-size: var(--text-xs);
	}

	.turns {
		display: grid;
		margin-left: calc(-1 * var(--space-6));
	}
</style>
