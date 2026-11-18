<script lang="ts">
	import { ShieldAlert } from '@lucide/svelte';
	import type { StatsQuery } from '#lib/api/query.ts';
	import type { LeakOut } from '#lib/api/types.ts';
	import type { LeakDecider } from '#lib/leaks/decide.svelte.ts';
	import HiddenLine from '#lib/leaks/HiddenLine.svelte';
	import LeakActions from '#lib/leaks/LeakActions.svelte';
	import { kindTitle, placeText } from '#lib/leaks/leaks.ts';

	/**
	 * One possible leak in a turn, any kind: a banner naming the kind, the value masked and where it
	 * sits, with Hide and Not sensitive (LeakActions). A value already hidden is one quiet line.
	 */
	let {
		leak,
		query = {},
		onchanged,
		now,
		hidden = leak.decided === 'hidden',
		decider
	}: {
		leak: LeakOut;
		/** Its value was hidden (it is in the board's `hidden` list). */
		hidden?: boolean;
		/** The filters the finding was found under (the session of the prompt). */
		query?: StatsQuery;
		onchanged?: () => void;
		/** Today, for the date of a hidden value (the year shows outside the current one). */
		now?: Date;
		decider?: LeakDecider;
	} = $props();
</script>

{#if hidden}
	<HiddenLine {leak} {now} />
{:else}
	<div class="banner" role="note" aria-label="Possible leak">
		<ShieldAlert size={16} aria-hidden="true" />
		<p class="text">
			<strong>Possible leak: {kindTitle(leak.kind)}</strong>
			<span><code>{leak.masked}</code> {placeText(leak.place)}</span>
		</p>
		<div class="actions">
			<LeakActions id={leak.id} {query} {onchanged} {decider} />
		</div>
	</div>
{/if}

<style>
	.banner {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr) auto;
		align-items: start;
		gap: var(--space-2) var(--space-3);
		padding: var(--space-3) var(--space-4);
		border: var(--border-hairline) solid var(--color-warning-border);
		border-radius: var(--radius-md);
		background: var(--color-warning-soft);
		color: var(--color-warning-text);
		font-size: var(--text-sm);
	}

	.banner > :global(svg) {
		margin-top: 2px;
	}

	.text {
		display: grid;
		gap: var(--space-0_5);
		color: var(--color-text);
	}

	code {
		font-family: var(--font-mono);
		overflow-wrap: anywhere;
	}

	.actions {
		display: grid;
		gap: var(--space-2);
	}

	@media (max-width: 40rem) {
		.banner {
			grid-template-columns: auto minmax(0, 1fr);
		}

		.actions {
			grid-column: 1 / -1;
		}
	}
</style>
