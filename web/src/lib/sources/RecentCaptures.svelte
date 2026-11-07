<script lang="ts">
	import { FolderGit2, GitBranch, MessagesSquare } from '@lucide/svelte';
	import { resolve } from '$app/paths';
	import type { RecentCapture } from '#lib/api/types.ts';
	import ConnectorBadge from '#lib/prompts/ConnectorBadge.svelte';
	import { modelName, plural, relative } from '#lib/shared/format.ts';

	/**
	 * The newest prompt of each tool and app it was typed in, and where GitPrompt put it: when,
	 * which project and branch, which session, which model. Type one prompt in each app, then look
	 * here: a missing row means nothing arrived; a row without an app means the tool doesn't say
	 * where it was typed. `signals` are the values the app rules read, shown so a surprise can be
	 * explained.
	 */
	let { captures }: { captures: RecentCapture[] } = $props();

	const signals = (c: RecentCapture) =>
		Object.entries(c.signals)
			.map(([k, v]) => `${k} = ${v}`)
			.join(', ');
</script>

<section class="recent" aria-labelledby="recent-title">
	<header>
		<h2 id="recent-title">Recent captures</h2>
		<p class="muted">
			The newest prompt from each tool and app. Type a prompt in an app, and it shows up here within
			seconds.
		</p>
	</header>
	{#if captures.length}
		<ul>
			{#each captures as c (`${c.connector.id}/${c.surface?.id ?? ''}`)}
				<li>
					<a class="row" href="{resolve('/prompts')}?id={c.id}">
						<span class="who">
							<ConnectorBadge connector={c.connector} surface={c.surface} />
							{#if !c.surface}<span class="unknown">App not reported</span>{/if}
						</span>
						<time datetime={c.received_at}>{relative(c.received_at)}</time>
						<span class="where">
							<span class="fact" title={c.project?.repo ?? 'No folder'}
								><FolderGit2 size={13} aria-hidden="true" />{c.project?.name ?? 'No folder'}</span
							>
							{#if c.branch}<span class="fact"
									><GitBranch size={13} aria-hidden="true" />{c.branch}{#if c.worktree}<span
											class="muted">&nbsp;(worktree)</span
										>{/if}</span
								>{/if}
							<span class="fact" title={c.session ?? 'No session'}
								><MessagesSquare size={13} aria-hidden="true" />{c.title ??
									(c.session ? `Session ${c.session.slice(0, 8)}` : 'No session')}</span
							>
							<span class="fact">{c.model ? modelName(c.model) : 'Model not reported'}</span>
						</span>
						<span class="count muted">{plural(c.prompts, 'prompt')}</span>
						{#if signals(c)}<code class="signals" title="What the app rules read">{signals(c)}</code
							>{/if}
					</a>
				</li>
			{/each}
		</ul>
	{:else}
		<p class="empty">Nothing captured yet. Connect a tool below, then type a prompt in it.</p>
	{/if}
</section>

<style>
	.recent {
		display: grid;
		gap: var(--space-3);
		margin-bottom: var(--space-6);
	}

	header {
		display: grid;
		gap: var(--space-1);
	}

	h2 {
		font-size: var(--text-md);
	}

	header p {
		max-width: var(--reading-max);
		font-size: var(--text-sm);
	}

	ul {
		display: grid;
		border-radius: var(--radius-lg);
		background: var(--color-surface);
		box-shadow: inset 0 0 0 1px var(--color-border);
	}

	li + li {
		border-top: var(--border-hairline) solid var(--color-border);
	}

	.row {
		display: grid;
		grid-template-columns: minmax(12rem, max-content) 6rem minmax(0, 1fr) auto;
		align-items: center;
		gap: var(--space-1) var(--space-4);
		padding: var(--space-2_5) var(--space-4);
		color: inherit;
		font-size: var(--text-sm);
		text-decoration: none;
	}

	.row:hover {
		background: var(--color-surface-hover);
	}

	.row:focus-visible {
		box-shadow: var(--focus-ring);
		border-radius: var(--radius-md);
	}

	.who {
		display: inline-flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
		min-width: 0;
	}

	.unknown {
		color: var(--color-text-muted);
		font-size: var(--text-xs);
	}

	time {
		color: var(--color-text-secondary);
		font-variant-numeric: tabular-nums;
	}

	.where {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1) var(--space-3);
		min-width: 0;
		color: var(--color-text-secondary);
	}

	.fact {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		min-width: 0;
		overflow-wrap: anywhere;
	}

	.fact :global(svg) {
		flex: none;
		color: var(--color-text-muted);
	}

	.count {
		font-size: var(--text-xs);
		white-space: nowrap;
	}

	.signals {
		grid-column: 3 / -1;
		color: var(--color-text-muted);
		font-size: var(--text-2xs);
		overflow-wrap: anywhere;
	}

	.empty {
		padding: var(--space-4);
		border-radius: var(--radius-lg);
		background: var(--color-surface);
		box-shadow: inset 0 0 0 1px var(--color-border);
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}

	@media (max-width: 48rem) {
		.row {
			grid-template-columns: minmax(0, 1fr) auto;
		}

		.where,
		.signals {
			grid-column: 1 / -1;
		}

		.count {
			display: none;
		}
	}
</style>
