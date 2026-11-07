<script lang="ts">
	import { CircleX, KeyRound, ShieldOff, Wrench } from '@lucide/svelte';
	import { resolve } from '$app/paths';
	import type { Attention } from '#lib/insights/aggregate.ts';
	import { excerpt, integer } from '#lib/shared/format.ts';
	import EmptyState from '#lib/ui/EmptyState.svelte';

	/** What deserves a second look, each with the way to review it. */
	let { attention }: { attention: Attention } = $props();

	const prompts = (query: string) => `${resolve('/prompts')}?${query}`;
	const rows = $derived(
		[
			{
				key: 'secrets',
				label: 'Possible secrets in prompts',
				hint: 'Hide them so they are not kept.',
				items: attention.secrets,
				icon: KeyRound,
				tone: 'warning',
				href: prompts('show=secrets&limit=200')
			},
			{
				key: 'failed',
				label: 'Turns that failed',
				hint: 'Ended on an API error.',
				items: attention.failed,
				icon: CircleX,
				tone: 'danger',
				href: prompts('show=failed&limit=200')
			},
			{
				key: 'tools',
				label: 'Turns with failed tool calls',
				hint: 'A command or edit did not go through.',
				items: attention.toolFailures,
				icon: Wrench,
				tone: 'neutral',
				href: undefined
			},
			{
				key: 'denied',
				label: 'Turns where a tool was refused',
				hint: 'Permission denied, or you rejected it.',
				items: attention.denied,
				icon: ShieldOff,
				tone: 'neutral',
				href: undefined
			}
		].filter((r) => r.items.length)
	);
</script>

{#if rows.length}
	<ul class="attention">
		{#each rows as row (row.key)}
			{@const latest = row.items[0]}
			<li class="tone-{row.tone}">
				<row.icon size={16} aria-hidden="true" />
				<div class="text">
					<p class="label">
						<span class="n num">{integer(row.items.length)}</span>
						{row.label}
					</p>
					<a class="latest truncate" href="{resolve('/prompts')}?id={latest.id}">{excerpt(latest.prompt, 90) || row.hint}</a>
				</div>
				{#if row.href}<a class="review" href={row.href}>Review</a>{/if}
			</li>
		{/each}
	</ul>
{:else}
	<EmptyState compact title="Nothing needs a look">
		No secrets, failed turns or refused tools in this window.
	</EmptyState>
{/if}

<style>
	.attention {
		display: grid;
		gap: var(--space-1);
	}

	li {
		--fg: var(--color-text-muted);
		display: grid;
		grid-template-columns: auto minmax(0, 1fr) auto;
		align-items: start;
		gap: var(--space-3);
		padding: var(--space-2) 0;
		border-bottom: var(--border-hairline) solid var(--color-border-subtle);
	}

	li:last-child {
		border-bottom: 0;
	}

	li > :global(svg) {
		margin-top: 2px;
		color: var(--fg);
	}

	.tone-warning {
		--fg: var(--color-warning-text);
	}

	.tone-danger {
		--fg: var(--color-danger-text);
	}

	.text {
		display: grid;
		min-width: 0;
	}

	.label {
		font-size: var(--text-sm);
		font-weight: var(--weight-medium);
	}

	.n {
		margin-right: var(--space-1);
		color: var(--fg);
		font-weight: var(--weight-semibold);
	}

	.latest {
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
		text-decoration: none;
	}

	.latest:hover {
		color: var(--color-accent-text);
		text-decoration: underline;
	}

	.review {
		font-size: var(--text-sm);
		font-weight: var(--weight-medium);
	}
</style>
