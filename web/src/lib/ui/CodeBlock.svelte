<script lang="ts">
	import CopyButton from '#lib/ui/CopyButton.svelte';

	/**
	 * Preformatted text with a copy button. `diff` colors unified-diff lines (+ added, - removed),
	 * with the sign kept in the text so the meaning never depends on color.
	 */
	let {
		code,
		label,
		wrap = true,
		diff = false,
		maxHeight = '24rem',
		copy = true
	}: {
		code: string;
		/** Shown in the top bar, e.g. a file name or "bash". */
		label?: string;
		wrap?: boolean;
		diff?: boolean;
		maxHeight?: string;
		copy?: boolean;
	} = $props();

	const lineKind = (line: string) =>
		line.startsWith('+++') || line.startsWith('---')
			? 'file'
			: line.startsWith('@@')
				? 'hunk'
				: line.startsWith('+')
					? 'add'
					: line.startsWith('-')
						? 'del'
						: 'ctx';
</script>

<div class="code-block">
	{#if label}
		<div class="bar">
			<span class="label mono">{label}</span>
			{#if copy}<span class="copy"><CopyButton text={code} label="Copy {label}" /></span>{/if}
		</div>
	{:else if copy}
		<span class="copy floating"><CopyButton text={code} /></span>
	{/if}
	<!-- tabindex: a scrollable region must be reachable by keyboard -->
	<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
	<pre
		class={[wrap && 'wrap', !label && copy && 'room']}
		style:max-height={maxHeight}
		tabindex="0">{#if diff}{#each code.split('\n') as line, i (i)}<span
					class="line {lineKind(line)}">{line || ' '}</span
				>{/each}{:else}{code}{/if}</pre>
</div>

<style>
	.code-block {
		position: relative;
		min-width: 0;
		border-radius: var(--radius-md);
		background: var(--color-code-bg);
		box-shadow: inset 0 0 0 1px var(--color-border-subtle);
	}

	.bar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
		min-height: var(--space-8);
		padding: var(--space-1) var(--space-1) 0 var(--space-3);
	}

	.label {
		color: var(--color-text-muted);
		font-size: var(--text-xs);
	}

	.copy {
		margin-left: auto;
	}

	.floating {
		position: absolute;
		top: var(--space-1);
		right: var(--space-1);
		z-index: 1;
		opacity: 0;
		transition: opacity var(--duration-fast) var(--ease-out);
	}

	.code-block:hover .floating,
	.floating:focus-within {
		opacity: 1;
	}

	@media (hover: none) {
		.floating {
			opacity: 1;
		}
	}

	pre {
		margin: 0;
		padding: var(--space-2) var(--space-3) var(--space-3);
		overflow: auto;
		color: var(--color-text);
		font-size: var(--text-sm);
		line-height: var(--leading-normal);
	}

	pre.room {
		padding-top: var(--space-2);
		padding-right: var(--space-10);
	}

	pre.wrap {
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}

	pre:focus-visible {
		box-shadow: inset var(--focus-ring);
	}

	.line {
		display: block;
		margin: 0 calc(-1 * var(--space-3));
		padding: 0 var(--space-3);
	}

	.add {
		background: var(--color-diff-add-bg);
		color: var(--color-diff-add-text);
	}

	.del {
		background: var(--color-diff-del-bg);
		color: var(--color-diff-del-text);
	}

	.hunk,
	.file {
		color: var(--color-text-muted);
	}
</style>
