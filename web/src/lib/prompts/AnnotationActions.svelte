<script lang="ts">
	import { Star, ThumbsDown, ThumbsUp } from '@lucide/svelte';
	import type { Annotation } from '#lib/api/types.ts';
	import Button from '#lib/ui/Button.svelte';

	/** Star and rate a prompt: toggle buttons, each saved right away. */
	let {
		value,
		save,
		busy = false
	}: { value: Annotation; save: (patch: Partial<Annotation>) => void; busy?: boolean } = $props();
</script>

<div class="actions" role="group" aria-label="Rate this prompt">
	<span class={['star', value.starred && 'on']}>
		<Button
			variant="ghost"
			size="sm"
			icon={Star}
			label={value.starred ? 'Starred' : 'Star'}
			pressed={value.starred}
			disabled={busy}
			onclick={() => save({ starred: !value.starred })}
		/>
	</span>
	<Button
		variant="ghost"
		size="sm"
		icon={ThumbsUp}
		label="Worked well"
		pressed={value.rating === 1}
		disabled={busy}
		onclick={() => save({ rating: value.rating === 1 ? 0 : 1 })}
	/>
	<span class={['down', value.rating === -1 && 'on']}>
		<Button
			variant="ghost"
			size="sm"
			icon={ThumbsDown}
			label="Didn't work"
			pressed={value.rating === -1}
			disabled={busy}
			onclick={() => save({ rating: value.rating === -1 ? 0 : -1 })}
		/>
	</span>
</div>

<style>
	.actions {
		display: inline-flex;
		gap: 2px;
	}

	.star.on :global(.btn) {
		background: var(--color-warning-soft);
		border-color: var(--color-warning-border);
		color: var(--color-star-edge);
	}

	.star.on :global(svg) {
		fill: var(--color-star);
	}

	.down.on :global(.btn) {
		background: var(--color-danger-soft);
		border-color: var(--color-danger-border);
		color: var(--color-danger-text);
	}
</style>
