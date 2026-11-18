<script lang="ts">
	import type { CommitGroup, CommitGrouping } from '#lib/api/types.ts';
	import { groupTitle } from '#lib/projects/commits.ts';
	import CommitTraceRow from '#lib/projects/CommitTraceRow.svelte';
	import { plural } from '#lib/shared/format.ts';

	/** The commits under a heading per phase, branch or day, newest group first. */
	let {
		groups,
		by,
		remote,
		now
	}: { groups: CommitGroup[]; by: CommitGrouping; remote?: string; now: Date } = $props();

	const id = $props.id();
</script>

{#each groups as group, i (group.key)}
	{@const title = groupTitle(group.key, by, now)}
	<section class="group" aria-labelledby="{id}-{i}">
		<h3 id="{id}-{i}" class="caption">
			{title} <span class="count">· {plural(group.count, 'commit')}</span>
		</h3>
		<ol class="rows">
			{#each group.commits as trace (trace.commit.hash)}
				<CommitTraceRow {trace} {remote} {now} />
			{/each}
		</ol>
	</section>
{/each}

<style>
	.group + .group {
		margin-top: var(--space-6);
	}

	.caption {
		margin: 0 0 var(--space-1);
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
