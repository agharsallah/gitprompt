<script lang="ts">
	import {
		AtSign,
		Braces,
		CircleHelp,
		GitBranch,
		Link2,
		ShieldAlert,
		TextCursorInput,
		X
	} from '@lucide/svelte';
	import { onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import { api } from '#lib/api/client.ts';
	import type { Annotation, PromptDetail, Source } from '#lib/api/types.ts';
	import { tokensOf, tokenTotal } from '#lib/insights/aggregate.ts';
	import type { Outcome } from '#lib/insights/outcome.ts';
	import { projectOf } from '#lib/insights/project.ts';
	import { compact, count, duration, integer, money, plural, when } from '#lib/shared/format.ts';
	import { Task } from '#lib/shared/task.svelte.ts';
	import Badge from '#lib/ui/Badge.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import CodeBlock from '#lib/ui/CodeBlock.svelte';
	import CopyButton from '#lib/ui/CopyButton.svelte';
	import Disclosure from '#lib/ui/Disclosure.svelte';
	import KeyValue from '#lib/ui/KeyValue.svelte';
	import RelativeTime from '#lib/ui/RelativeTime.svelte';
	import Tabs from '#lib/ui/Tabs.svelte';
	import ActivityPanel from '#lib/prompts/ActivityPanel.svelte';
	import AnnotationActions from '#lib/prompts/AnnotationActions.svelte';
	import EventList from '#lib/prompts/EventList.svelte';
	import GitPanel from '#lib/prompts/GitPanel.svelte';
	import NotesEditor from '#lib/prompts/NotesEditor.svelte';
	import OutcomeBadge from '#lib/prompts/OutcomeBadge.svelte';
	import { injectedSummary, originOf, typedText } from '#lib/prompts/origin.ts';
	import SessionPanel from '#lib/prompts/SessionPanel.svelte';
	import TurnTimeline from '#lib/prompts/TurnTimeline.svelte';
	import UsagePanel from '#lib/prompts/UsagePanel.svelte';
	import { assistantOf, availability, toolOf } from '#lib/sources/capabilities.ts';
	import { projectId } from '#lib/stories/story.ts';
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

	// The prompt's tool, from the connectors (spec data): who answers, and whether it can report usage.
	let sources = $state<Source[]>([]);
	onMount(() => {
		api.sources().then(
			(list) => (sources = list),
			() => {} // the names fall back to "Agent"; nothing else depends on them
		);
	});
	const tool = $derived(toolOf(sources, prompt.source));
	const agent = $derived(assistantOf(tool));
	const usage = $derived(tool && availability(tool.layers, 'usage'));

	const s = $derived(prompt.summary);
	const st = $derived(prompt.stats);
	const project = $derived(projectOf(prompt));
	const session = $derived(prompt.fields.session_id);
	const secrets = $derived(st.secrets ?? []);
	const tokens = $derived(tokenTotal(tokensOf(prompt)));
	const origin = $derived(originOf(prompt));
	const text = $derived(typedText(prompt));
	const injected = $derived(injectedSummary(prompt.injected ?? []));

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

	const redaction = new Task();
	async function hideSecrets() {
		const question =
			'Replace the detected secrets in everything stored for this prompt? This cannot be undone.';
		if (confirm(question) && (await redaction.run(() => api.redact(prompt.id)))) onchange();
	}

	// Long texts fold, so the story around them stays in view.
	const isLong = (text?: string | null) =>
		!!text && (text.length > 1400 || text.split('\n').length > 18);
	let expanded = $state(false);
	let promptExpanded = $state(false);
	const longResponse = $derived(isLong(s.response));
	const longPrompt = $derived(isLong(text));

	const textFacts = $derived(
		[
			{
				icon: TextCursorInput,
				text: `${plural(st.words ?? 0, 'word')}, about ${compact(st.approx_tokens ?? 0)} tokens`
			},
			st.questions && { icon: CircleHelp, text: plural(st.questions, 'question') },
			st.code_blocks && { icon: Braces, text: plural(st.code_blocks, 'code block') },
			st.urls?.length && { icon: Link2, text: plural(st.urls.length, 'link') },
			st.at_mentions?.length && { icon: AtSign, text: st.at_mentions.map((m) => `@${m}`).join(' ') }
		].filter((f) => !!f) as { icon: typeof Braces; text: string }[]
	);

	const figures = $derived(
		[
			s.response_s != null && { label: 'Worked for', value: duration(s.response_s) },
			{
				label: 'Tool calls',
				value: integer(count(s.tool_calls)),
				hint: s.tool_failures ? `${s.tool_failures} failed` : undefined
			},
			{ label: 'Files changed', value: integer(s.files_written?.length ?? 0) },
			s.cost_usd && { label: 'Cost', value: money(s.cost_usd) },
			s.api_requests && { label: 'Tokens', value: compact(tokens) }
		].filter((f) => !!f) as { label: string; value: string; hint?: string }[]
	);

	type TabId = 'activity' | 'usage' | 'git' | 'session' | 'versions' | 'events' | 'raw';
	let tab = $state<TabId>('activity');
	const tabs = $derived([
		{ id: 'activity' as const, label: 'Activity', count: count(s.tool_calls) },
		{ id: 'usage' as const, label: 'Usage' },
		{ id: 'git' as const, label: 'Repository', count: prompt.commits.commits?.length },
		{ id: 'session' as const, label: 'Session', disabled: !session },
		{ id: 'versions' as const, label: 'Versions', disabled: origin.system },
		{ id: 'events' as const, label: 'Events', count: prompt.events.length },
		{ id: 'raw' as const, label: 'Raw' }
	]);

	// Everything the tool sent that the spec did not map, so new metadata shows up without code changes.
	const extra = $derived(
		Object.fromEntries(
			Object.entries(prompt.raw).filter(([, v]) => !Object.values(prompt.fields).includes(v))
		)
	);
	const captured = $derived(
		Object.fromEntries(Object.entries(prompt.fields).filter(([k]) => k !== 'prompt'))
	);
</script>

<article class="detail" aria-labelledby="prompt-{prompt.id}-title">
	<header class="top">
		<div class="where">
			<OutcomeBadge {outcome} />
			<span class="time" title={when(prompt.received_at)}
				><RelativeTime iso={prompt.received_at} /></span
			>
			{#if session && s.turn}
				<a href="{resolve('/sessions/[id]', { id: session })}#prompt-{prompt.id}">
					Turn {s.turn} of its session
				</a>
			{/if}
		</div>
		<div class="actions">
			<AnnotationActions value={prompt.annotation} {save} busy={task.busy} />
			<CopyButton text={text ?? ''} label="Copy prompt" ghost />
			<Button
				variant="ghost"
				size="sm"
				icon={X}
				label="Close"
				href={closeHref}
				data-sveltekit-reset="false"
			/>
		</div>
	</header>

	<div class="project-line">
		{#if project.root}
			<a
				class="project"
				href="{resolve('/projects/[id]', { id: projectId(project.root) })}#prompt-{prompt.id}"
				>{project.name}</a
			>
		{:else}
			<span class="project">{project.name}</span>
		{/if}
		{#if prompt.context.git_branch}
			<span class="branch"
				><GitBranch size={13} aria-hidden="true" />{prompt.context.git_branch}</span
			>
		{/if}
		{#if prompt.fields.permission_mode}<Badge size="sm" variant="outline"
				>{prompt.fields.permission_mode} mode</Badge
			>{/if}
		{#if s.slash_command ?? st.slash_command}<Badge size="sm" tone="accent"
				>{s.slash_command ?? st.slash_command}</Badge
			>{/if}
		{#if prompt.context.tool_version}<span class="muted version"
				>v{prompt.context.tool_version}</span
			>{/if}
	</div>

	<section
		class={['voice', 'you', origin.system && 'system']}
		aria-labelledby="prompt-{prompt.id}-title"
	>
		<h2 id="prompt-{prompt.id}-title" class="who" title={origin.system ? origin.hint : undefined}>
			{#if origin.system}
				<origin.icon size={14} aria-hidden="true" />{origin.label}
			{:else}
				<span class="avatar" aria-hidden="true"></span>You
			{/if}
		</h2>
		<p class={['said', longPrompt && !promptExpanded && 'folded']}>
			{text ?? 'No prompt text was captured: the source mapping may be outdated.'}
		</p>
		{#if longPrompt}
			<button
				class="more you-more"
				type="button"
				aria-expanded={promptExpanded}
				onclick={() => (promptExpanded = !promptExpanded)}
			>
				{promptExpanded ? 'Show less' : 'Show the whole prompt'}
			</button>
		{/if}
		{#if injected}
			<div class="injected">
				<Disclosure summary="Context added by the tool">
					{#snippet meta()}{injected}{/snippet}
					<CodeBlock code={prompt.prompt ?? ''} label="Prompt as received" maxHeight="20rem" />
				</Disclosure>
			</div>
		{/if}
		<ul class="text-facts" aria-label="About the prompt text">
			{#each textFacts as f (f.text)}<li><f.icon size={13} aria-hidden="true" />{f.text}</li>{/each}
		</ul>
	</section>

	{#if secrets.length}
		<Callout tone="warning" icon={ShieldAlert} title="Possible secret in this prompt">
			Detected: {secrets.join(', ')}. The value is stored only on this machine; hiding it replaces
			it everywhere it was stored for this prompt.
			{#snippet actions()}
				<Button size="sm" variant="secondary" onclick={hideSecrets} loading={redaction.busy}
					>Hide the secret</Button
				>
			{/snippet}
		</Callout>
	{/if}
	{#if task.error || redaction.error}<Callout tone="danger" live
			>{task.error || redaction.error}</Callout
		>{/if}

	<section class="turn" aria-label="How the turn went">
		<dl class="figures">
			{#each figures as f (f.label)}
				<div>
					<dt>{f.label}</dt>
					<dd>
						{f.value}{#if f.hint}<span class="hint">{f.hint}</span>{/if}
					</dd>
				</div>
			{/each}
		</dl>
		<TurnTimeline
			events={prompt.events}
			start={prompt.received_at}
			responseS={s.response_s}
			kinds={s.tool_kind_of}
		/>
	</section>

	{#if s.response}
		<section class="voice agent" aria-labelledby="prompt-{prompt.id}-answer">
			<h2 id="prompt-{prompt.id}-answer" class="who">
				<span class="avatar" aria-hidden="true"></span>{agent}
			</h2>
			<div class={['said', 'response', longResponse && !expanded && 'folded']}>{s.response}</div>
			{#if longResponse}
				<button
					class="more"
					type="button"
					aria-expanded={expanded}
					onclick={() => (expanded = !expanded)}
				>
					{expanded ? 'Show less' : 'Show the whole answer'}
				</button>
			{/if}
		</section>
	{/if}
	{#if s.error}
		<Callout tone="danger" title="The turn ended on an error">
			<code>{typeof s.error === 'string' ? s.error : JSON.stringify(s.error)}</code>
		</Callout>
	{/if}

	<section class="notes" aria-label="Your notes">
		<NotesEditor value={prompt.annotation} {save} {saved} />
	</section>

	<Tabs {tabs} bind:selected={tab} label="Prompt details">
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
				<div class="raw">
					<section>
						<h3>Captured fields</h3>
						<KeyValue data={captured} rawKeys />
					</section>
					{#if Object.keys(extra).length}
						<section>
							<h3>Other payload fields</h3>
							<KeyValue data={extra} rawKeys />
						</section>
					{/if}
					<CodeBlock
						code={JSON.stringify(prompt.raw, null, 2)}
						label="Raw payload"
						maxHeight="28rem"
					/>
				</div>
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

	.top {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
	}

	.where {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2) var(--space-3);
		font-size: var(--text-sm);
	}

	.time {
		color: var(--color-text-secondary);
	}

	.actions {
		display: flex;
		align-items: center;
		gap: var(--space-1);
	}

	.project-line {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2) var(--space-3);
		margin-top: calc(-1 * var(--space-2));
		font-size: var(--text-sm);
	}

	.project {
		color: var(--color-text);
		font-weight: var(--weight-semibold);
		text-decoration: none;
	}

	a.project:hover {
		color: var(--color-accent-text);
		text-decoration: underline;
	}

	.branch {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
		color: var(--color-text-secondary);
	}

	.version {
		font-size: var(--text-xs);
	}

	/* The two voices of a turn: your words on a lagoon rule, the agent's on an iris one. */
	.voice {
		--voice: var(--color-accent);
		display: grid;
		gap: var(--space-2);
		padding-left: var(--space-4);
		border-left: 3px solid var(--voice);
	}

	.voice.agent {
		--voice: var(--color-agent);
	}

	/* Not typed by you: a quiet rule and text, with the origin's icon and label as the speaker. */
	.voice.system {
		--voice: var(--color-border-strong);
	}

	.system .said {
		color: var(--color-text-secondary);
		font-size: var(--text-md);
	}

	.you.system .said {
		font-weight: var(--weight-regular);
	}

	.injected {
		max-width: var(--reading-max);
		font-size: var(--text-sm);
	}

	.who {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
		font-stretch: var(--stretch-normal);
		letter-spacing: var(--tracking-normal);
	}

	.avatar {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--voice);
	}

	.said {
		max-width: var(--reading-max);
		font-size: var(--text-lg);
		line-height: var(--leading-relaxed);
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}

	.you .said {
		font-weight: var(--weight-medium);
	}

	.response {
		font-size: var(--text-md);
	}

	.folded {
		max-height: 22rem;
		overflow: hidden;
		mask-image: linear-gradient(to bottom, #000 75%, transparent);
	}

	.more {
		justify-self: start;
		color: var(--color-agent-text);
		font-size: var(--text-sm);
		font-weight: var(--weight-medium);
	}

	.you-more {
		color: var(--color-accent-text);
	}

	.more:hover {
		text-decoration: underline;
	}

	.text-facts {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1) var(--space-4);
		color: var(--color-text-muted);
		font-size: var(--text-xs);
	}

	.text-facts li {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
	}

	.turn {
		display: grid;
		gap: var(--space-4);
		padding: var(--space-4);
		border-radius: var(--radius-lg);
		background: var(--color-surface-sunken);
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
		display: flex;
		align-items: baseline;
		gap: var(--space-1_5);
		margin: 0;
		font-size: var(--text-xl);
		font-weight: var(--weight-semibold);
		font-stretch: var(--stretch-heading);
	}

	.hint {
		color: var(--color-danger-text);
		font-size: var(--text-xs);
		font-weight: var(--weight-medium);
	}

	.notes {
		padding: var(--space-4);
		border-radius: var(--radius-lg);
		box-shadow: inset 0 0 0 1px var(--color-border);
	}

	.raw {
		display: grid;
		gap: var(--space-5);
	}

	.raw h3 {
		margin-bottom: var(--space-2);
		font-size: var(--text-md);
	}
</style>
