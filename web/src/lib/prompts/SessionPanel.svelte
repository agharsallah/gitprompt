<script lang="ts">
	import { FileText, FolderInput, Settings2 } from '@lucide/svelte';
	import { resolve } from '$app/paths';
	import type { PromptDetail } from '#lib/api/types.ts';
	import {
		duration,
		humanize,
		integer,
		plural,
		present,
		relativePath
	} from '#lib/shared/format.ts';
	import BarList from '#lib/ui/charts/BarList.svelte';
	import Disclosure from '#lib/ui/Disclosure.svelte';
	import KeyValue from '#lib/ui/KeyValue.svelte';
	import RelativeTime from '#lib/ui/RelativeTime.svelte';

	/** The session this prompt belongs to: its span, what it loaded, what telemetry counted. */
	let { prompt }: { prompt: PromptDetail } = $props();

	const s = $derived(prompt.session);
	/** SessionStart.source, in words: how the session began, then how it was taken up again. */
	const HOW: Record<string, string> = {
		startup: 'new',
		resume: 'resumed',
		clear: 'cleared',
		compact: 'compacted',
		fork: 'forked'
	};
	const how = (source: string) => HOW[source] ?? humanize(source).toLowerCase();
	const starts = $derived((s.starts ?? []).map(how));
	const startedAs = $derived(starts.length ? ` (${starts[0]})` : '');
	const again = $derived(starts.slice(1));
	const id = $derived(prompt.fields.session_id);
	const span = $derived(
		s.first_event_at && s.last_event_at
			? (Date.parse(s.last_event_at) - Date.parse(s.first_event_at)) / 1000
			: null
	);
	const metrics = $derived(
		Object.fromEntries(
			Object.entries(s.metric_totals ?? {}).map(([k, v]) => [k.replace(/^claude_code\./, ''), v])
		)
	);
	const highlights = $derived(
		[
			['Lines added', metrics['lines_of_code.count[added]']],
			['Lines removed', metrics['lines_of_code.count[removed]']],
			['Commits', metrics['commit.count']],
			['Pull requests', metrics['pull_request.count']],
			['Your active time', metrics['active_time.total[user]'], 'time'],
			['Tool active time', metrics['active_time.total[cli]'], 'time']
		].filter(([, v]) => typeof v === 'number') as [string, number, string?][]
	);
	const events = $derived(
		Object.entries(s.events ?? {})
			.sort((a, b) => b[1] - a[1])
			.map(([name, value]) => ({ key: name, label: name, value, mono: true }))
	);
	const lists = $derived(
		[
			{ label: 'Instruction files loaded', items: s.instructions_loaded ?? [], icon: FileText },
			{ label: 'Directory changes', items: present(s.cwd_changes), icon: FolderInput },
			{ label: 'Config changes', items: present(s.config_changes), icon: Settings2 }
		].filter((l) => l.items.length)
	);
</script>

<div class="session">
	<section class="block">
		<div class="span-line">
			{#if s.first_event_at}
				<span>Started <RelativeTime iso={s.first_event_at} />{startedAs}</span>
			{/if}
			{#if again.length}
				<span>taken up again {plural(again.length, 'time')}: {again.join(', ')}</span>
			{/if}
			{#if span !== null}<span>active over {duration(span)}</span>{/if}
			{#if s.ended}<span>ended: {humanize(s.ended).toLowerCase()}</span>{/if}
		</div>
		{#if id}
			<p class="links">
				<a href="{resolve('/prompts')}?session={encodeURIComponent(id)}"
					>Every prompt in this session</a
				>
				<a href="{resolve('/sessions')}#session-{id}">See it among your sessions</a>
			</p>
		{/if}
	</section>

	{#if highlights.length}
		<dl class="figures">
			{#each highlights as [label, value, kind] (label)}
				<div>
					<dt>{label}</dt>
					<dd>{kind === 'time' ? duration(value) : integer(value)}</dd>
				</div>
			{/each}
		</dl>
	{/if}

	{#each lists as list (list.label)}
		<section class="block">
			<h3>{list.label} <span class="muted num">{list.items.length}</span></h3>
			<ul class="paths">
				{#each list.items as item, i (i)}
					<li>
						<list.icon size={14} aria-hidden="true" /><span class="mono" title={item}
							>{relativePath(item)}</span
						>
					</li>
				{/each}
			</ul>
		</section>
	{/each}

	{#if s.subagent_results?.length}
		<section class="block">
			<h3>Subagents that finished <span class="muted num">{s.subagent_results.length}</span></h3>
			<ul class="results">
				{#each s.subagent_results as r, i (i)}
					<li>
						<span class="who"
							>{r.agent_type || 'Subagent'}{#if r.stop_reason}<span class="muted"
									>, {humanize(r.stop_reason).toLowerCase()}</span
								>{/if}</span
						>
						{#if r.answer}<span class="answer">{r.answer}</span>{/if}
					</li>
				{/each}
			</ul>
		</section>
	{/if}

	{#if prompt.fields.transcript_path}
		<p class="transcript muted">
			<FileText size={14} aria-hidden="true" />Transcript kept by the tool (never read by
			GitPrompt):
			<span class="mono" title={prompt.fields.transcript_path}
				>{relativePath(prompt.fields.transcript_path)}</span
			>
		</p>
	{/if}

	{#if events.length}
		<Disclosure summary="Events received for this session">
			{#snippet meta()}<span class="num">{integer(events.reduce((a, e) => a + e.value, 0))}</span
				>{/snippet}
			<BarList items={events} label="Session events by type" max={12} otherLabel="Other events" />
		</Disclosure>
	{/if}

	{#if Object.keys(metrics).length}
		<Disclosure summary="Telemetry metric totals">
			<KeyValue data={metrics} rawKeys />
		</Disclosure>
	{/if}

	{#if s.environment && Object.keys(s.environment).length}
		<Disclosure summary="Environment">
			<KeyValue data={s.environment} rawKeys />
		</Disclosure>
	{/if}
</div>

<style>
	.session {
		display: grid;
		gap: var(--space-5);
	}

	.block {
		display: grid;
		gap: var(--space-2);
	}

	.results {
		display: grid;
		gap: var(--space-2);
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.results li {
		display: grid;
		gap: 2px;
		padding-left: var(--space-3);
		box-shadow: inset 2px 0 0 var(--color-agent-border);
	}

	.who {
		font-size: var(--text-sm);
		font-weight: var(--weight-medium);
	}

	.answer {
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}

	.transcript {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1);
		margin: 0;
		font-size: var(--text-sm);
		overflow-wrap: anywhere;
	}

	h3 {
		display: flex;
		gap: var(--space-2);
		font-size: var(--text-md);
	}

	h3 .muted {
		font-weight: var(--weight-regular);
	}

	.span-line {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1) var(--space-4);
		color: var(--color-text-secondary);
	}

	.links {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-4);
		font-size: var(--text-sm);
	}

	.figures {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(8rem, 1fr));
		gap: var(--space-4);
		margin: 0;
	}

	.figures div {
		display: grid;
		gap: var(--space-0_5);
		padding-left: var(--space-3);
		border-left: var(--border-thick) solid var(--color-border);
	}

	dt {
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}

	dd {
		margin: 0;
		font-size: var(--text-xl);
		font-weight: var(--weight-semibold);
		font-stretch: var(--stretch-heading);
	}

	.paths {
		display: grid;
		gap: var(--space-1);
	}

	.paths li {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-width: 0;
		font-size: var(--text-xs);
	}

	.paths :global(svg) {
		color: var(--color-text-muted);
	}

	.paths .mono {
		min-width: 0;
		overflow-wrap: anywhere;
	}
</style>
