<script lang="ts">
	import { FilePen, GitCommitHorizontal, Timer, Wrench } from '@lucide/svelte';
	import type { Prompt } from '#lib/api/types.ts';
	import { count, duration, modelName, money } from '#lib/shared/format.ts';

	/**
	 * What a turn amounted to, in a single quiet line: how long the tool worked, how many tool calls,
	 * files changed, cost and model. Each fact carries an icon and a tooltip that says what it is.
	 */
	let { prompt, models = true }: { prompt: Prompt; models?: boolean } = $props();

	const s = $derived(prompt.summary);
	const tools = $derived(count(s.tool_calls));
	const files = $derived(s.files_written?.length ?? 0);
	const mainModel = $derived(Object.entries(s.models ?? {}).sort((a, b) => b[1] - a[1])[0]?.[0]);
</script>

<span class="facts">
	{#if s.response_s != null}
		<span class="fact" title="Time until the tool finished">
			<Timer size={13} aria-hidden="true" /><span class="sr-only">Worked for</span>{duration(
				s.response_s
			)}
		</span>
	{/if}
	{#if tools}
		<span class="fact" title="Tool calls{s.tool_failures ? `, ${s.tool_failures} failed` : ''}">
			<Wrench size={13} aria-hidden="true" />{tools}<span class="sr-only"> tool calls</span>
			{#if s.tool_failures}<span class="failures"
					>{s.tool_failures}<span class="sr-only"> failed</span></span
				>{/if}
		</span>
	{/if}
	{#if files}
		<span class="fact" title="Files edited or written">
			<FilePen size={13} aria-hidden="true" />{files}<span class="sr-only"> files changed</span>
		</span>
	{/if}
	{#if s.bash_commands?.some((c) => /\bgit\s+commit\b/.test(c))}
		<span class="fact" title="Ran git commit">
			<GitCommitHorizontal size={13} aria-hidden="true" /><span class="sr-only">Committed</span>
		</span>
	{/if}
	{#if s.cost_usd}
		<span class="fact" title="Cost of this turn (telemetry)">{money(s.cost_usd)}</span>
	{/if}
	{#if models && mainModel}
		<span class="fact model" title="Model used for most requests">{modelName(mainModel)}</span>
	{/if}
</span>

<style>
	.facts {
		display: inline-flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1) var(--space-3);
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
		font-variant-numeric: tabular-nums;
	}

	.fact {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
		white-space: nowrap;
	}

	.fact :global(svg) {
		color: var(--color-text-muted);
	}

	.failures {
		margin-left: 2px;
		padding: 0 4px;
		border-radius: var(--radius-full);
		background: var(--color-danger-soft);
		color: var(--color-danger-text);
		font-weight: var(--weight-semibold);
	}

	.model {
		color: var(--color-agent-text);
	}
</style>
