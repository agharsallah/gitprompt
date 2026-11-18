<script lang="ts">
	import type { StatsQuery } from '#lib/api/query.ts';
	import type { Leaks } from '#lib/api/types.ts';
	import HiddenLine from '#lib/leaks/HiddenLine.svelte';
	import { groupByKind, kindTitle, leaksState } from '#lib/leaks/leaks.ts';
	import LeakRow from '#lib/overview/LeakRow.svelte';
	import { plural } from '#lib/shared/format.ts';
	import { LEAKS_ANCHOR } from '#lib/shared/links.ts';
	import Disclosure from '#lib/ui/Disclosure.svelte';
	import Panel from '#lib/ui/Panel.svelte';

	/**
	 * The possible leaks of the range, where Worth a look and the key figure lead (`#leaks`): open
	 * findings grouped by kind, each masked with Hide and Not sensitive; "All clear" with what was
	 * checked when there are none. Values hidden are one quiet line each; dismissed ones fold under
	 * "Dismissed · N", each with Undo.
	 */
	let {
		leaks,
		query = {},
		onchanged,
		now
	}: {
		leaks: Leaks;
		/** The Overview's filters: the findings were found under them. */
		query?: StatsQuery;
		/** A decision changed the stored data: the page reloads. */
		onchanged?: () => void;
		/** Today, for the dates of hidden values. */
		now?: Date;
	} = $props();

	const groups = $derived(groupByKind(leaks.open));
	const decided = $derived(leaks.hidden.length + leaks.dismissed.length > 0);
</script>

{#snippet footer()}
	<div class="decided">
		{#each leaks.hidden as leak (leak.id)}
			<HiddenLine {leak} {now} />
		{/each}
		{#if leaks.dismissed.length}
			<Disclosure summary="Dismissed · {leaks.dismissed.length}">
				<p class="muted note">
					Marked not sensitive: {plural(leaks.dismissed.length, 'value')} no longer flagged.
				</p>
				<ul class="rows">
					{#each leaks.dismissed as leak (leak.id)}
						<LeakRow {leak} {query} {onchanged} dismissed />
					{/each}
				</ul>
			</Disclosure>
		{/if}
	</div>
{/snippet}

<Panel
	id={LEAKS_ANCHOR}
	title="Possible leaks"
	pad="sm"
	state={leaksState(leaks)}
	footer={decided ? footer : undefined}
>
	<div class="groups">
		{#each groups as group (group.kind)}
			<section class="group" aria-label={group.kind}>
				<h3>{kindTitle(group.kind)} <span class="count">{group.findings.length}</span></h3>
				<ul class="rows">
					{#each group.findings as leak (leak.id)}
						<LeakRow {leak} {query} {onchanged} />
					{/each}
				</ul>
			</section>
		{/each}
	</div>
</Panel>

<style>
	.groups,
	.decided {
		display: grid;
		gap: var(--space-4);
	}

	.decided {
		gap: var(--space-1);
		margin-top: var(--space-3);
	}

	.group {
		display: grid;
		gap: var(--space-2);
	}

	h3 {
		font-size: var(--text-sm);
		font-weight: var(--weight-semibold);
	}

	.count {
		color: var(--color-text-muted);
		font-weight: var(--weight-regular);
	}

	.rows {
		display: grid;
		gap: var(--space-2);
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.note {
		margin-bottom: var(--space-2);
		font-size: var(--text-sm);
	}
</style>
