<script lang="ts">
	import type { Resume } from '#lib/api/types.ts';
	import type { UsageAction } from '#lib/shared/usage-actions.ts';
	import CodeBlock from '#lib/ui/CodeBlock.svelte';
	import CopyButton from '#lib/ui/CopyButton.svelte';
	import { resumeLabel } from '#lib/sessions/resume.ts';

	/**
	 * How to pick a session up again: the line to paste (`cd <folder> && <command>`, built and
	 * shell-quoted by the daemon) with a copy button, or (`compact`, for a row) a button that shows
	 * the tool's command in code (`⧉ claude --resume`), with the whole line in its tooltip.
	 * Nothing without a resume command. A copy is counted (`copy.resume`, or what `count` says where
	 * the copy is its own action, as Pick up's `pickup.copy`).
	 */
	let {
		resume,
		compact = false,
		count = 'copy.resume'
	}: { resume?: Resume | null; compact?: boolean; count?: UsageAction } = $props();
</script>

{#if resume}
	{#if compact}
		<CopyButton
			text={resume.paste}
			label="Copy resume command"
			caption={resumeLabel(resume)}
			mono
			hint="Copy the command that resumes this session in a terminal: {resume.paste}"
			{count}
		/>
	{:else}
		<CodeBlock code={resume.paste} label="Resume in a terminal" maxHeight="8rem" {count} />
	{/if}
{/if}
