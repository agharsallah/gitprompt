<script lang="ts">
	import { Search } from '@lucide/svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { api } from '#lib/api/client.ts';
	import type { CommitGrouping, CommitTrace } from '#lib/api/types.ts';
	import Deferred from '#lib/async/Deferred.svelte';
	import { Resource } from '#lib/async/resource.svelte.ts';
	import { copyBranch } from '#lib/projects/branch.ts';
	import BranchPrompts from '#lib/projects/BranchPrompts.svelte';
	import CommitGroups from '#lib/projects/CommitGroups.svelte';
	import CommitSummary from '#lib/projects/CommitSummary.svelte';
	import {
		COMMIT_ANCHOR,
		commitAnchor,
		commitsState,
		openRow,
		rowAction,
		showing,
		type CommitShow
	} from '#lib/projects/commits.ts';
	import { clock } from '#lib/shared/clock.svelte.ts';
	import { promptHref } from '#lib/shared/links.ts';
	import { track } from '#lib/shared/usage.ts';
	import ListSkeleton from '#lib/ui/ListSkeleton.svelte';
	import PanelState from '#lib/ui/PanelState.svelte';
	import SegmentedControl from '#lib/ui/SegmentedControl.svelte';
	import TextField from '#lib/ui/TextField.svelte';
	import ToggleChip from '#lib/ui/ToggleChip.svelte';

	/**
	 * Every commit of a project with the prompt it followed: searched by subject, file or hash,
	 * grouped by phase, branch or day. ↑ ↓ move between rows, ↵ opens the prompt, ⌘↵ the commit on
	 * its forge in a new tab (when the daemon knows its page; the hash is plain text without one).
	 * The prompts behind `branch` (the project's current one) copy as Markdown for a pull request;
	 * by branch, each group copies its own.
	 */
	let { project, branch }: { project: string; branch?: string } = $props();

	const GROUPINGS: { value: CommitGrouping; label: string }[] = [
		{ value: 'phase', label: 'Phase' },
		{ value: 'branch', label: 'Branch' },
		{ value: 'day', label: 'Day' }
	];

	let q = $state('');
	let group = $state<CommitGrouping>('phase');
	let show = $state<CommitShow>('all');
	let attempt = $state(0);
	// "Try again" asks again: the attempt is part of the key, which the resource reads with the source.
	const commits = new Resource(
		() => api.projectCommits(project, { q, group }),
		() => `${project} ${attempt}`
	);
	const now = $derived(new Date(clock.now));
	const notGit = $derived(commits.value?.git === false);

	let list = $state<HTMLElement>();
	const traces = $derived((commits.value?.groups ?? []).flatMap((g) => g.commits));
	const traceAt = (row: HTMLElement): CommitTrace | undefined =>
		traces.find((t) => commitAnchor(t.commit.hash) === row.id);

	const opener = {
		url: (href: string) => void window.open(href, '_blank', 'noopener'),
		prompt: (id: number) => {
			track('commit.open.prompt');
			void goto(promptHref(id));
		}
	};

	function onkeydown(event: KeyboardEvent) {
		const target = event.target;
		if (!list || !(target instanceof HTMLElement) || !list.contains(target)) return;
		const rows = [...list.querySelectorAll<HTMLElement>('li[id]')];
		const row = target.closest<HTMLElement>('li[id]');
		const action = rowAction(event, row ? rows.indexOf(row) : null, rows.length, target === row);
		if (!action) return;
		if (action.kind === 'move') rows[action.to].focus();
		else if (row) {
			const trace = traceAt(row);
			if (trace) openRow(action.kind, trace, opener);
		}
		event.preventDefault();
	}

	// A link to a commit's row (`#commit-<hash>`) lands on it once the commits are drawn.
	let landed = '';
	$effect(() => {
		const hash = page.url.hash;
		if (!list || !traces.length || hash === landed || !hash.startsWith(`#${COMMIT_ANCHOR}`)) return;
		const row = document.getElementById(hash.slice(1));
		if (!row) return;
		landed = hash;
		row.scrollIntoView({ block: 'center' });
		row.focus({ preventScroll: true });
	});
</script>

<svelte:window {onkeydown} />

{#if !notGit}
	<div class="toolbar">
		<TextField
			label="Search commits"
			hideLabel
			icon={Search}
			type="search"
			placeholder="Search subject, file or hash"
			clearable
			bind:value={q}
		/>
		<SegmentedControl options={GROUPINGS} bind:value={group} label="Group commits by" size="sm" />
		<ToggleChip bind:checked={() => show === 'prompt', (only) => (show = only ? 'prompt' : 'all')}
			>Only from prompts</ToggleChip
		>
	</div>
{/if}

<Deferred
	resource={commits}
	label="Loading the commits"
	what="the commits"
	onretry={() => attempt++}
>
	{#snippet pending()}<ListSkeleton variant="rows" />{/snippet}
	{#snippet children(data)}
		{@const groups = showing(data.groups, show)}
		{#if data.git && data.counts.total}<CommitSummary counts={data.counts} bind:show />{/if}
		{#if data.git && branch}<BranchPrompts {project} {branch} />{/if}
		<PanelState state={commitsState(data, groups.length, q)}>
			<div bind:this={list}>
				<CommitGroups {groups} by={data.group} {now} copy={(b) => void copyBranch(project, b)} />
			</div>
		</PanelState>
	{/snippet}
</Deferred>

<style>
	.toolbar {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2) var(--space-3);
		margin-bottom: var(--space-4);
	}

	.toolbar > :global(.field) {
		flex: 1 1 16rem;
		max-width: 22rem;
	}
</style>
