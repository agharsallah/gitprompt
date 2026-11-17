<script lang="ts">
	import { Bot, GitBranch, ShieldCheck, X } from '@lucide/svelte';
	import type { Annotation, PromptDetail } from '#lib/api/types.ts';
	import type { Outcome } from '#lib/insights/outcome.ts';
	import { projectOf } from '#lib/insights/project.ts';
	import { when } from '#lib/shared/format.ts';
	import Badge from '#lib/ui/Badge.svelte';
	import Button from '#lib/ui/Button.svelte';
	import CopyButton from '#lib/ui/CopyButton.svelte';
	import RelativeTime from '#lib/ui/RelativeTime.svelte';
	import AnnotationActions from '#lib/prompts/AnnotationActions.svelte';
	import ConnectorBadge from '#lib/prompts/ConnectorBadge.svelte';
	import type { FactsView } from '#lib/prompts/facts.ts';
	import OutcomeBadge from '#lib/prompts/OutcomeBadge.svelte';
	import { typedText } from '#lib/prompts/origin.ts';
	import { projectHref, sessionHref } from '#lib/shared/links.ts';
	import { anchor, projectId } from '#lib/stories/story.ts';
	import LinkArrow from '#lib/ui/LinkArrow.svelte';

	/**
	 * Where a prompt sits: how it ended, when, its turn in the session, and the actions on it; then
	 * the tool, model, project, branch and mode it ran with.
	 */
	let {
		prompt,
		outcome,
		view,
		closeHref,
		save,
		busy
	}: {
		prompt: PromptDetail;
		outcome: Outcome;
		view: FactsView;
		closeHref: string;
		save: (patch: Partial<Annotation>) => void;
		busy: boolean;
	} = $props();

	const s = $derived(prompt.summary);
	const project = $derived(projectOf(prompt));
	const session = $derived(prompt.fields.session_id);
	const slashCommand = $derived(s.slash_command ?? prompt.stats.slash_command);
</script>

<header class="top">
	<div class="where">
		<OutcomeBadge {outcome} />
		<span class="time" title={when(prompt.received_at)}
			><RelativeTime iso={prompt.received_at} /></span
		>
		{#if session && s.turn}
			<LinkArrow
				href={sessionHref(session, { anchor: anchor(prompt.id) })}
				title="Session › Turn {s.turn}"
			>
				Turn {s.turn} of its session
			</LinkArrow>
		{/if}
	</div>
	<div class="actions">
		<AnnotationActions value={prompt.annotation} {save} {busy} />
		<CopyButton text={typedText(prompt) ?? ''} label="Copy prompt" ghost count="copy.prompt" />
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
	<ConnectorBadge connector={prompt.facts.connector} surface={prompt.facts.surface} size="md" />
	{#if view.model}
		<Badge tone="agent" icon={Bot} title={view.model.hint}>{view.model.name}</Badge>
	{/if}
	{#if project.root}
		<LinkArrow
			href={projectHref(projectId(project.root), { anchor: anchor(prompt.id) })}
			title="Project › {project.name}">{project.name}</LinkArrow
		>
	{:else}
		<span class="project">{project.name}</span>
	{/if}
	{#if prompt.context.git_branch}
		<span class="branch"><GitBranch size={13} aria-hidden="true" />{prompt.context.git_branch}</span
		>
	{/if}
	{#if view.permissionMode}
		<Badge
			size="sm"
			variant="outline"
			icon={ShieldCheck}
			title="Permission mode the tool reported with the prompt">{view.permissionMode} mode</Badge
		>
	{/if}
	{#if slashCommand}<Badge size="sm" tone="accent">{slashCommand}</Badge>{/if}
	{#if prompt.context.tool_version}
		<span class="muted version">v{prompt.context.tool_version}</span>
	{/if}
</div>

<style>
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

	.branch {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
		color: var(--color-text-secondary);
	}

	.version {
		font-size: var(--text-xs);
	}
</style>
