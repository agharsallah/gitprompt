<script lang="ts">
	import type { PickUp } from '#lib/api/types.ts';
	import { PICK_UP_MAX, stoppedText } from '#lib/overview/stopped.ts';
	import ResumeCommand from '#lib/sessions/ResumeCommand.svelte';
	import { clock } from '#lib/shared/clock.svelte.ts';
	import { sessionHref, sessionsHref } from '#lib/shared/links.ts';
	import LinkArrow from '#lib/ui/LinkArrow.svelte';
	import Panel from '#lib/ui/Panel.svelte';
	import { readyOr, type PanelState } from '#lib/ui/panelState.ts';

	/**
	 * The sessions to pick up again, running ones first (the daemon's order): how each stopped and
	 * the command that resumes it. At most three, then the way to all of them.
	 */
	let {
		rows,
		now
	}: {
		/** The daemon's rows; undefined while they are on their way (a skeleton). */
		rows?: readonly PickUp[];
		/** A fixed "now"; default: the shared clock. */
		now?: number;
	} = $props();

	const shown = $derived(rows?.slice(0, PICK_UP_MAX) ?? []);
	const at = $derived(now ?? clock.now);
	const state: PanelState = $derived(
		rows
			? readyOr(shown.length, {
					kind: 'empty',
					title: 'Nothing to pick up',
					message: 'No session is running or was left recently.'
				})
			: { kind: 'loading', label: 'Loading the sessions to pick up', body: 'rows', rows: 3 }
	);
</script>

{#snippet all()}
	<LinkArrow href={sessionsHref()} title="Sessions">All sessions</LinkArrow>
{/snippet}

<Panel title="Pick up" pad="sm" {state} footer={rows ? all : undefined}>
	<ul class="rows">
		{#each shown as row (row.session)}
			{@const name = row.name ?? row.label ?? 'Untitled session'}
			<li class="row">
				<span class={['glyph', row.presence]} aria-hidden="true"></span>
				<a class="what" href={sessionHref(row.session)} title="Session › {name}">
					<span class="name">{name}</span>
					<span class="meta">
						{#if row.project}<span class="project">{row.project}</span>{/if}
						<span class={['stopped', row.presence]}>{stoppedText(row, at)}</span>
					</span>
				</a>
				<ResumeCommand compact resume={row.resume} count="pickup.copy" />
			</li>
		{/each}
	</ul>
</Panel>

<style>
	.rows {
		display: grid;
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.row {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		padding: var(--space-2) 0;
	}

	.row + .row {
		border-top: var(--border-hairline) solid var(--color-border);
	}

	/* Running: the filled green dot of the running badge; left: an open ring. */
	.glyph {
		flex: none;
		width: 8px;
		height: 8px;
		border-radius: var(--radius-full);
		box-shadow: inset 0 0 0 1.5px var(--color-text-muted);
	}

	.glyph.running {
		background: var(--color-success-solid);
		box-shadow: none;
	}

	.what {
		display: grid;
		flex: 1;
		min-width: 0;
		color: inherit;
		text-decoration: none;
	}

	.what:hover .name {
		color: var(--color-accent-text);
		text-decoration: underline;
	}

	.name {
		overflow: hidden;
		font-weight: var(--weight-medium);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.meta {
		display: flex;
		flex-wrap: wrap;
		gap: 0 var(--space-2);
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
	}

	.project {
		color: var(--color-text);
	}

	.stopped.running {
		color: var(--color-success-text);
	}
</style>
