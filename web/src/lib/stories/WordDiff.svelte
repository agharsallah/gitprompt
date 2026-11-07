<script lang="ts">
	import type { DiffOp } from '#lib/api/types.ts';

	/**
	 * How a prompt's wording changed, inline: removed words struck through, added words underlined,
	 * each also announced in words (`<del>`/`<ins>` and hidden labels), so it never relies on colour.
	 */
	let { diff }: { diff: DiffOp[] } = $props();
</script>

<p class="diff">
	{#each diff as [op, text], i (i)}{#if op === 'insert'}<ins
				><span class="sr-only">added: </span>{text}</ins
			>{:else if op === 'delete'}<del><span class="sr-only">removed: </span>{text}</del>{:else}<span
				>{text}</span
			>{/if}{/each}
</p>

<style>
	.diff {
		max-width: var(--reading-max);
		line-height: var(--leading-relaxed);
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}

	ins {
		border-radius: 2px;
		background: var(--color-diff-add-bg);
		color: var(--color-diff-add-text);
		text-decoration: underline;
		text-decoration-thickness: 1px;
		text-underline-offset: 3px;
	}

	del {
		border-radius: 2px;
		background: var(--color-diff-del-bg);
		color: var(--color-diff-del-text);
		text-decoration: line-through;
	}
</style>
