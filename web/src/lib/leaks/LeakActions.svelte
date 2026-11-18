<script lang="ts">
	import type { StatsQuery } from '#lib/api/query.ts';
	import { decideWithApi, type LeakDecider } from '#lib/leaks/decide.svelte.ts';
	import HideValues from '#lib/leaks/HideValues.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';

	/**
	 * The two actions on a possible leak, the same wherever it shows (the Overview, the reader):
	 * Hide, which asks once in place (HideValues) since it rewrites the stored text, and Not
	 * sensitive, at once. A dismissed finding has one action instead: Undo. `onchanged` runs after
	 * each decision (the page reloads its leaks).
	 */
	let {
		id,
		query = {},
		dismissed = false,
		onchanged,
		decider = decideWithApi(id, { query, onchanged })
	}: {
		/** The finding. */
		id: string;
		/** The filters it was found under (the board Hide reads). */
		query?: StatsQuery;
		dismissed?: boolean;
		onchanged?: () => void;
		/** The decisions; default: over the daemon. Tests pass their own. */
		decider?: LeakDecider;
	} = $props();
</script>

{#if decider.asking}
	<HideValues
		findings={[id]}
		{query}
		flow={decider.asking}
		ondone={(n) => decider.hidden(n)}
		oncancel={() => decider.cancel()}
	/>
{:else}
	<div class="actions">
		{#if dismissed}
			<Button
				size="sm"
				variant="ghost"
				loading={decider.task.busy}
				onclick={() => decider.undo()}
				title="Show it as a possible leak again">Undo</Button
			>
		{:else}
			<Button size="sm" onclick={() => decider.ask()} disabled={decider.task.busy}>Hide</Button>
			<Button
				size="sm"
				variant="ghost"
				loading={decider.task.busy}
				onclick={() => decider.dismiss()}
				title="Never flag this value again">Not sensitive</Button
			>
		{/if}
	</div>
{/if}
{#if decider.task.error}<Callout tone="danger" live>{decider.task.error}</Callout>{/if}

<style>
	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}
</style>
