<script lang="ts">
	import type { Resume } from '#lib/api/types.ts';
	import CodeBlock from '#lib/ui/CodeBlock.svelte';
	import CopyButton from '#lib/ui/CopyButton.svelte';

	/**
	 * How to pick a session up again: the line to paste (`cd <folder> && <command>`, built and
	 * shell-quoted by the daemon) with a copy button, or only the button (`compact`, for a row).
	 * Nothing without a resume command. A copy is counted (`copy.resume`).
	 */
	let { resume, compact = false }: { resume?: Resume | null; compact?: boolean } = $props();
</script>

{#if resume}
	{#if compact}
		<span class="compact" title={resume.paste}>
			<CopyButton text={resume.paste} label="Copy resume command" count="copy.resume" />
		</span>
	{:else}
		<CodeBlock
			code={resume.paste}
			label="Resume in a terminal"
			maxHeight="8rem"
			count="copy.resume"
		/>
	{/if}
{/if}

<style>
	.compact {
		display: inline-flex;
	}
</style>
