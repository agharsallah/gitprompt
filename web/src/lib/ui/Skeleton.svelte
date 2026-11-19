<script lang="ts">
	/**
	 * A placeholder shaped like the content that is on its way. Decorative: the region that holds it
	 * says what is loading (lib/async/Loading.svelte). The shimmer stops under reduced motion.
	 */
	let {
		width = '100%',
		height = '0.875rem',
		shape = 'line',
		lines = 1
	}: {
		width?: string;
		height?: string;
		/** line: a line of text; block: a chart or card; circle: an avatar or dot. */
		shape?: 'line' | 'block' | 'circle';
		/** Several lines of text, the last one shorter. */
		lines?: number;
	} = $props();
</script>

{#if lines > 1}
	<span class="lines" style:width aria-hidden="true">
		{#each { length: lines }, i (i)}
			<span class="skeleton line" style:height style:width={i === lines - 1 ? '62%' : '100%'}
			></span>
		{/each}
	</span>
{:else}
	<span
		class={['skeleton', shape]}
		style:width={shape === 'circle' ? height : width}
		style:height
		aria-hidden="true"
	></span>
{/if}

<style>
	.skeleton {
		display: block;
		flex: none;
		max-width: 100%;
		background: linear-gradient(
				90deg,
				transparent 0%,
				color-mix(in oklab, var(--color-surface-raised) 55%, transparent) 50%,
				transparent 100%
			)
			var(--color-surface-hover);
		background-repeat: no-repeat;
		background-size: 40% 100%;
		animation: shimmer 1.6s var(--ease-in-out) infinite;
	}

	.line {
		border-radius: var(--radius-xs);
	}

	.block {
		border-radius: var(--radius-md);
	}

	.circle {
		border-radius: var(--radius-full);
	}

	.lines {
		display: grid;
		gap: var(--space-2);
		max-width: 100%;
	}

	@keyframes shimmer {
		from {
			background-position: -60% 0;
		}
		to {
			background-position: 160% 0;
		}
	}
</style>
