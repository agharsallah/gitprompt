<script lang="ts">
	import {
		CircleCheck,
		CircleDashed,
		CircleStop,
		CircleX,
		GitCommitHorizontal,
		Hourglass,
		Minus,
		Redo2,
		SquarePen
	} from '@lucide/svelte';
	import type { TurnOutcome } from '#lib/api/types.ts';
	import type { Doorway } from '#lib/insights/filters.ts';
	import { TURN_OUTCOMES } from '#lib/insights/outcome.ts';
	import type { Overview } from '#lib/insights/overview.ts';
	import { hasTelemetry, metric } from '#lib/insights/series.ts';
	import { integer, percent, plural } from '#lib/shared/format.ts';
	import BarList from '#lib/ui/charts/BarList.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import Panel from '#lib/ui/Panel.svelte';
	import Facts from './Facts.svelte';
	import type { Availability } from '#lib/sources/capabilities.ts';
	import CapabilityNote from './CapabilityNote.svelte';
	import NotAvailable from './NotAvailable.svelte';

	/** What came of the prompts: how turns ended, commits, lines of code, and prompts nobody followed up. */
	let { data, link, availability }: { data: Overview; link: Doorway; availability: Availability } =
		$props();

	const ORDER: TurnOutcome[] = ['done', 'failed', 'interrupted', 'superseded', 'running', 'open'];
	const ICONS = {
		done: CircleCheck,
		failed: CircleX,
		interrupted: CircleStop,
		superseded: Redo2,
		running: Hourglass,
		open: CircleDashed
	};
	const t = $derived(data.activity.totals);
	const count = (o: TurnOutcome) => data.outcomes.groups.find((g) => g.key === o)?.prompts ?? 0;
	const lastOfSession = $derived(t.prompts - t.followed);
	const tel = $derived(data.telemetry);
	const added = $derived(metric(tel, 'linesAdded'));
	const removed = $derived(metric(tel, 'linesRemoved'));
	const commits = $derived(metric(tel, 'commits'));
</script>

<Panel
	title="What came of my prompts?"
	description="Superseded: no end was recorded and you sent another prompt, so it was interrupted or abandoned."
>
	{#if t.prompts}
		<div class="stack">
			<BarList
				label="Prompts by how their turn ended"
				items={ORDER.filter((o) => count(o)).map((o) => ({
					key: o,
					label: TURN_OUTCOMES[o].label,
					value: count(o),
					icon: ICONS[o],
					href: link({ outcome: [o] })
				}))}
				format={(v) => `${integer(v)} (${percent(v / t.prompts)})`}
			/>
			<Facts
				label="Follow-up and commits"
				facts={[
					{
						label: 'Last prompt of their session',
						value: integer(lastOfSession),
						icon: Minus,
						hint: 'no prompt came after it: the conversation ended there',
						href: lastOfSession ? link({ has: ['no_follow_up'] }) : undefined
					},
					{
						label: 'Led to a commit by the agent',
						value: integer(t.with_commit),
						icon: GitCommitHorizontal,
						hint: `${plural(t.git_commits, 'git commit')} run from its shell`,
						href: t.with_commit ? link({ has: ['commit'] }) : undefined
					}
				]}
			/>
			<h3 class="sub">In these sessions (telemetry)</h3>
			{#if hasTelemetry(tel)}
				<Facts
					label="Code changed, from telemetry"
					facts={[
						{
							label: 'Lines of code added',
							value: added !== null ? integer(added) : '—',
							icon: SquarePen,
							hint: removed !== null ? `${integer(removed)} removed` : undefined
						},
						{
							label: 'Commits',
							value: commits !== null ? integer(commits) : '—',
							icon: GitCommitHorizontal,
							hint: `over ${plural(tel.sessions, 'session')}`
						}
					]}
				/>
				<CapabilityNote {availability} />
			{:else}
				<NotAvailable what="lines of code and commits" {availability} />
			{/if}
		</div>
	{:else}
		<EmptyState compact title="No prompts in this window" />
	{/if}
</Panel>

<style>
	.stack {
		display: grid;
		gap: var(--space-3);
	}

	.sub {
		margin-top: var(--space-2);
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
		font-weight: var(--weight-medium);
		font-stretch: var(--stretch-normal);
		letter-spacing: var(--tracking-normal);
	}
</style>
