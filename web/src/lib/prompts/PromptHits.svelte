<script lang="ts">
	import type { HitRow } from '#lib/prompts/hits.ts';
	import SearchHit from '#lib/stories/SearchHit.svelte';

	/** The prompts matching some words: each with its matching words in context. */
	let {
		rows,
		selectedId,
		hrefFor
	}: {
		rows: HitRow[];
		selectedId?: number | null;
		/** Where a hit opens: the page with the filters kept and the prompt selected. */
		hrefFor: (id: number) => string;
	} = $props();
</script>

<ol class="hits" aria-label="Search results">
	{#each rows as { hit, text } (hit.id)}
		<SearchHit
			{hit}
			href={hrefFor(hit.id)}
			selected={selectedId === hit.id}
			copy={text ?? undefined}
		/>
	{/each}
</ol>
