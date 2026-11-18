<script lang="ts">
	import type { StatsQuery } from '#lib/api/query.ts';
	import type { LeakOut } from '#lib/api/types.ts';
	import LeakActions from '#lib/leaks/LeakActions.svelte';
	import { placeText } from '#lib/leaks/leaks.ts';
	import { promptHref } from '#lib/shared/links.ts';

	/**
	 * One finding on the Overview: its value masked, where it sits, a link to its turn in the
	 * reader, and its actions (Hide and Not sensitive; Undo once dismissed).
	 */
	let {
		leak,
		query = {},
		dismissed = false,
		onchanged
	}: { leak: LeakOut; query?: StatsQuery; dismissed?: boolean; onchanged?: () => void } = $props();
</script>

<li class="row">
	<p class="what">
		<code>{leak.masked}</code>
		<span class="where">
			{dismissed ? `${leak.kind}, ` : ''}{placeText(leak.place)} ·
			<a href={promptHref(leak.prompt)} title="Prompt">Open the turn</a>
		</span>
	</p>
	<LeakActions id={leak.id} {query} {dismissed} {onchanged} />
</li>

<style>
	.row {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2) var(--space-4);
	}

	.what {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: var(--space-1) var(--space-2);
		min-width: 0;
	}

	code {
		font-family: var(--font-mono);
		overflow-wrap: anywhere;
	}

	.where {
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}
</style>
