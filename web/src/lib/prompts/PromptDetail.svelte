<script lang="ts">
	import { onMount } from 'svelte';
	import { api } from '#lib/api/client.ts';
	import type { Annotation, PromptDetail, Source } from '#lib/api/types.ts';
	import type { Outcome } from '#lib/insights/outcome.ts';
	import { count, inline } from '#lib/shared/format.ts';
	import { Task } from '#lib/shared/task.svelte.ts';
	import { track } from '#lib/shared/usage.ts';
	import type { ReaderTab } from '#lib/shared/usage-actions.ts';
	import Callout from '#lib/ui/Callout.svelte';
	import Tabs from '#lib/ui/Tabs.svelte';
	import ActivityPanel from '#lib/prompts/ActivityPanel.svelte';
	import EventList from '#lib/prompts/EventList.svelte';
	import { factsView, missingTokens } from '#lib/prompts/facts.ts';
	import GitPanel from '#lib/prompts/GitPanel.svelte';
	import NotesEditor from '#lib/prompts/NotesEditor.svelte';
	import { originOf } from '#lib/prompts/origin.ts';
	import PromptHeader from '#lib/prompts/PromptHeader.svelte';
	import RawPanel from '#lib/prompts/RawPanel.svelte';
	import SecretWarning from '#lib/prompts/SecretWarning.svelte';
	import SessionPanel from '#lib/prompts/SessionPanel.svelte';
	import TurnOverview from '#lib/prompts/TurnOverview.svelte';
	import UsagePanel from '#lib/prompts/UsagePanel.svelte';
	import Voice from '#lib/prompts/Voice.svelte';
	import WhatCameOfIt from '#lib/prompts/WhatCameOfIt.svelte';
	import YourPrompt from '#lib/prompts/YourPrompt.svelte';
	import { availability, toolOf } from '#lib/sources/capabilities.ts';
	import VersionsPanel from '#lib/stories/VersionsPanel.svelte';

	/**
	 * One prompt as a story: what you asked, how the turn went, what the agent answered, and your
	 * notes; then the detail behind it in tabs.
	 */
	let {
		prompt,
		outcome,
		closeHref,
		onchange
	}: { prompt: PromptDetail; outcome: Outcome; closeHref: string; onchange: () => void } = $props();

	// The prompt's tool, from the connectors (spec data): whether it can report usage.
	let sources = $state<Source[]>([]);
	onMount(() => {
		api.sources().then(
			(list) => (sources = list),
			() => {} // only the note on missing tokens depends on them
		);
	});
	const tool = $derived(toolOf(sources, prompt.source));
	const usage = $derived(tool && availability(tool.layers, 'usage'));
	const view = $derived(factsView(prompt.facts));
	const noTokens = $derived(missingTokens(prompt.facts, usage));

	const s = $derived(prompt.summary);
	const secrets = $derived(prompt.stats.secrets ?? []);
	const response = $derived(s.response == null ? null : inline(s.response));

	// Annotation edits are saved right away; the parent reloads and passes the new value down.
	const task = new Task();
	let saved = $state(false);
	async function save(patch: Partial<Annotation>) {
		saved = false;
		if (await task.run(() => api.annotate(prompt.id, { ...prompt.annotation, ...patch }))) {
			saved = 'note' in patch;
			onchange();
		}
	}

	type TabId = ReaderTab;
	let tab = $state<TabId>('activity');
	const tabs = $derived([
		{ id: 'activity' as const, label: 'Activity', count: count(s.tool_calls) },
		{ id: 'usage' as const, label: 'Usage' },
		{ id: 'git' as const, label: 'Repository', count: prompt.commits.commits?.length },
		{ id: 'session' as const, label: 'Session', disabled: !prompt.fields.session_id },
		{ id: 'versions' as const, label: 'Versions', disabled: originOf(prompt).system },
		{ id: 'events' as const, label: 'Events', count: prompt.events.length },
		{ id: 'raw' as const, label: 'Raw' }
	]);
</script>

<article class="detail" aria-labelledby="prompt-{prompt.id}-title">
	<PromptHeader {prompt} {outcome} {view} {closeHref} {save} busy={task.busy} />

	<YourPrompt {prompt} headingId="prompt-{prompt.id}-title" />

	{#if secrets.length}<SecretWarning id={prompt.id} {secrets} {onchange} />{/if}
	{#if task.error}<Callout tone="danger" live>{task.error}</Callout>{/if}

	<TurnOverview {prompt} {view} {noTokens} />

	{#if response}
		<Voice
			id="prompt-{prompt.id}-answer"
			tone="agent"
			text={response}
			moreLabel="Show the whole answer"
			speaker={view.assistant}
		/>
	{/if}
	{#if s.error}
		<Callout tone="danger" title="The turn ended on an error">
			<code>{typeof s.error === 'string' ? s.error : JSON.stringify(s.error)}</code>
		</Callout>
	{/if}

	<WhatCameOfIt {prompt} id="prompt-{prompt.id}-came-of-it" />

	<section class="notes" aria-label="Your notes">
		<NotesEditor value={prompt.annotation} {save} {saved} />
	</section>

	<Tabs
		{tabs}
		bind:selected={tab}
		onselect={(id) => track(`reader.tab.${id}`)}
		label="Prompt details"
	>
		{#snippet panel(id)}
			{#if id === 'activity'}
				<ActivityPanel {prompt} />
			{:else if id === 'usage'}
				<UsagePanel {prompt} availability={usage} />
			{:else if id === 'git'}
				<GitPanel {prompt} />
			{:else if id === 'session'}
				<SessionPanel {prompt} />
			{:else if id === 'versions'}
				<VersionsPanel id={prompt.id} />
			{:else if id === 'events'}
				<EventList events={prompt.events} start={prompt.received_at} kinds={s.tool_kind_of} />
			{:else}
				<RawPanel {prompt} />
			{/if}
		{/snippet}
	</Tabs>
</article>

<style>
	.detail {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		gap: var(--space-5);
		min-width: 0;
	}

	.notes {
		padding: var(--space-4);
		border-radius: var(--radius-lg);
		box-shadow: inset 0 0 0 1px var(--color-border);
	}
</style>
