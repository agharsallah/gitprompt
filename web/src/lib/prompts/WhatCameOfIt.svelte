<script lang="ts">
	import { FilePen } from '@lucide/svelte';
	import { cameOfIt, type TurnResult } from '#lib/prompts/cameOfIt.ts';

	/**
	 * What the turn produced, right after the answer: one sentence, the files it wrote first, and
	 * the commits that followed, each opening its row on the project's Commits tab (cameOfIt.ts).
	 */
	let { prompt, id }: { prompt: TurnResult; id: string } = $props();

	const view = $derived(cameOfIt(prompt));
</script>

<section class="came" aria-labelledby={id}>
	<h2 {id} class="label">What came of it</h2>
	<p class="sentence">{view.sentence}</p>
	{#if !view.empty}
		{#if view.files.length}
			<ul class="files" aria-label="Files written">
				{#each view.files as file (file)}
					<li><FilePen size={14} aria-hidden="true" /><span class="mono">{file}</span></li>
				{/each}
				{#if view.more}<li class="more muted">and {view.more} more</li>{/if}
			</ul>
		{/if}
		{#if view.commits.length}
			<ul class="commits" aria-label="Commits after">
				{#each view.commits as { commit, href } (commit.hash)}
					<li>
						{#if href}
							<a {href}><code class="hash">{commit.hash}</code> {commit.subject}</a>
						{:else}
							<code class="hash">{commit.hash}</code> {commit.subject}
						{/if}
					</li>
				{/each}
			</ul>
		{:else}
			<p class="muted">No commit followed</p>
		{/if}
	{/if}
</section>

<style>
	.came {
		display: grid;
		gap: var(--space-2);
	}

	.label {
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
		font-weight: var(--weight-medium);
	}

	.sentence {
		font-size: var(--text-md);
		font-weight: var(--weight-semibold);
		font-variant-numeric: tabular-nums;
	}

	.files,
	.commits {
		display: grid;
		gap: var(--space-1);
		font-size: var(--text-sm);
	}

	.files li {
		display: flex;
		align-items: center;
		gap: var(--space-1_5);
		min-width: 0;
		overflow-wrap: anywhere;
	}

	.files li :global(svg) {
		flex: none;
		color: var(--color-text-muted);
	}

	.commits li {
		overflow-wrap: anywhere;
	}

	.hash {
		color: var(--color-accent-text);
		font-size: var(--text-xs);
	}
</style>
