<script lang="ts">
	import { Copy } from '@lucide/svelte';
	import type { CommitGroup, CommitGrouping } from '#lib/api/types.ts';
	import { groupTitle } from '#lib/projects/commits.ts';
	import CommitTraceRow from '#lib/projects/CommitTraceRow.svelte';
	import { plural } from '#lib/shared/format.ts';
	import Button from '#lib/ui/Button.svelte';

	/**
	 * The commits under a heading per phase, branch or day, newest group first. By branch, each
	 * branch's heading offers `copy`: the prompts behind that branch.
	 */
	let {
		groups,
		by,
		now,
		copy
	}: {
		groups: CommitGroup[];
		by: CommitGrouping;
		now: Date;
		copy?: (branch: string) => void;
	} = $props();

	const id = $props.id();
</script>

{#each groups as group, i (group.key)}
	{@const title = groupTitle(group.key, by, now)}
	<section class="group" aria-labelledby="{id}-{i}">
		<header>
			<h3 id="{id}-{i}" class="caption">
				{title} <span class="count">· {plural(group.count, 'commit')}</span>
			</h3>
			{#if copy && by === 'branch' && group.key !== null}
				{@const branch = group.key}
				<Button size="sm" variant="ghost" icon={Copy} onclick={() => copy(branch)}
					>Copy prompts behind {branch}</Button
				>
			{/if}
		</header>
		<ol class="rows">
			{#each group.commits as trace (trace.commit.hash)}
				<CommitTraceRow {trace} {now} />
			{/each}
		</ol>
	</section>
{/each}

<style>
	.group + .group {
		margin-top: var(--space-6);
	}

	header {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
		margin: 0 0 var(--space-1);
	}

	.caption {
		margin: 0;
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
		font-weight: var(--weight-semibold);
		letter-spacing: 0.04em;
		text-transform: uppercase;
	}

	.count {
		color: var(--color-text-muted);
		font-weight: var(--weight-regular);
	}

	.rows {
		display: grid;
	}
</style>
