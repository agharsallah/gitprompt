<script lang="ts">
	import { resolve } from '$app/paths';
	import type { Prompt } from '#lib/api/types.ts';
	import { OUTCOMES, outcome } from '#lib/insights/outcome.ts';
	import { clock } from '#lib/shared/clock.svelte.ts';
	import { duration, excerpt } from '#lib/shared/format.ts';
	import Tooltip from '#lib/ui/charts/Tooltip.svelte';

	/**
	 * A session's turns side by side, oldest first: each column's height is how long the agent
	 * worked (square-root scale, so one long turn doesn't flatten the rest), its color how it ended.
	 * Each column links to its prompt.
	 */
	let { prompts }: { prompts: Prompt[] } = $props();

	const max = $derived(Math.max(...prompts.map((p) => p.summary.response_s ?? 0), 1));
	const height = (p: Prompt) => 4 + Math.sqrt((p.summary.response_s ?? 0) / max) * 28;
	const last = $derived(prompts.at(-1)?.id);

	let width = $state(0);
	let active = $state<{ p: Prompt; x: number } | null>(null);

	function show(event: Event, p: Prompt) {
		const el = event.currentTarget as HTMLElement;
		active = { p, x: el.offsetLeft + el.offsetWidth / 2 };
	}
	const describe = (p: Prompt, o: ReturnType<typeof outcome>) =>
		`Turn ${p.summary.turn ?? ''}: ${OUTCOMES[o].label.toLowerCase()}${p.summary.response_s != null ? `, ${duration(p.summary.response_s)}` : ''}. ${excerpt(p.prompt, 80)}`;
</script>

<div class="strip" bind:clientWidth={width}>
	{#each prompts as p (p.id)}
		{@const o = outcome(p, clock.now, p.id !== last)}
		<a
			class="turn {o}"
			href="{resolve('/prompts')}?id={p.id}"
			style:height="{height(p)}px"
			aria-label={describe(p, o)}
			onpointerenter={(e) => show(e, p)}
			onpointerleave={() => (active = null)}
			onfocus={(e) => show(e, p)}
			onblur={() => (active = null)}
		></a>
	{/each}
	{#if active}
		<Tooltip x={active.x} y={0} {width}>
			<strong
				>Turn {active.p.summary.turn}{active.p.summary.response_s != null
					? `, ${duration(active.p.summary.response_s)}`
					: ''}</strong
			>
			{excerpt(active.p.prompt, 90)}
		</Tooltip>
	{/if}
</div>

<style>
	.strip {
		position: relative;
		display: flex;
		flex-wrap: wrap;
		align-items: flex-end;
		gap: 2px;
		min-height: 32px;
	}

	.turn {
		flex: 0 0 auto;
		width: 8px;
		border-radius: 3px 3px 1px 1px;
		background: var(--color-accent);
	}

	.turn:hover,
	.turn:focus-visible {
		filter: brightness(1.15);
	}

	.turn:focus-visible {
		box-shadow: var(--focus-ring);
		border-radius: 3px;
	}

	.failed {
		background: var(--color-danger-solid);
	}

	.running {
		background: var(--color-agent);
	}

	.open {
		background: var(--color-border-control);
	}
</style>
