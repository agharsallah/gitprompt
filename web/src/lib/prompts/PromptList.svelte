<script lang="ts">
	import type { Prompt } from '#lib/api/types.ts';
	import { outcome, supersededIds } from '#lib/insights/outcome.ts';
	import { clock } from '#lib/shared/clock.svelte.ts';
	import { dayKey, dayLabel, plural } from '#lib/shared/format.ts';
	import PromptRow from '#lib/prompts/PromptRow.svelte';

	/**
	 * Prompts grouped by day, newest first, each on its session's rail: consecutive prompts of the
	 * same session are joined, so a conversation reads as one line and a new session starts a new one.
	 */
	let {
		prompts,
		selectedId,
		hrefFor,
		showProject = true
	}: {
		prompts: Prompt[];
		selectedId?: number | null;
		hrefFor: (p: Prompt) => string;
		showProject?: boolean;
	} = $props();

	const superseded = $derived(supersededIds(prompts));
	const days = $derived.by(() => {
		const groups: { key: string; prompts: Prompt[] }[] = [];
		for (const p of prompts) {
			const key = dayKey(p.received_at);
			if (groups.at(-1)?.key !== key) groups.push({ key, prompts: [] });
			groups.at(-1)!.prompts.push(p);
		}
		return groups;
	});
	const same = (a?: Prompt, b?: Prompt) =>
		!!a && !!b && !!a.fields.session_id && a.fields.session_id === b.fields.session_id;
</script>

<div class="prompt-list">
	{#each days as day (day.key)}
		<section aria-labelledby="day-{day.key}">
			<h2 id="day-{day.key}" class="day">
				{dayLabel(day.key, new Date(clock.now))}
				<span class="count">{plural(day.prompts.length, 'prompt')}</span>
			</h2>
			<ol>
				{#each day.prompts as p, i (p.id)}
					<li>
						<PromptRow
							prompt={p}
							outcome={outcome(p, clock.now, superseded.has(p.id))}
							href={hrefFor(p)}
							selected={selectedId === p.id}
							joinPrev={same(p, day.prompts[i - 1])}
							joinNext={same(p, day.prompts[i + 1])}
							{showProject}
						/>
					</li>
				{/each}
			</ol>
		</section>
	{/each}
</div>

<style>
	.prompt-list {
		display: grid;
		gap: var(--space-4);
	}

	.day {
		position: sticky;
		top: 0;
		z-index: 2;
		display: flex;
		align-items: baseline;
		gap: var(--space-2);
		padding: var(--space-2) var(--space-3);
		background: var(--color-surface);
		font-size: var(--text-sm);
		font-stretch: var(--stretch-normal);
		letter-spacing: var(--tracking-normal);
	}

	.count {
		color: var(--color-text-muted);
		font-size: var(--text-xs);
		font-weight: var(--weight-regular);
	}

	ol {
		display: grid;
	}
</style>
