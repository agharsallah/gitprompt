<script lang="ts">
	// Fonts are bundled, never fetched from a CDN: the dashboard makes no request off this machine.
	import '@fontsource-variable/instrument-sans/wdth.css';
	import '@fontsource/commit-mono/400.css';
	import '@fontsource/commit-mono/600.css';
	import '#lib/design/tokens.css';
	import '#lib/design/base.css';
	import { setLucideProps } from '@lucide/svelte';
	import { navigating } from '$app/state';
	import Sidebar from '#lib/layout/Sidebar.svelte';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();

	setLucideProps({ size: 16, strokeWidth: 1.75 });
</script>

<svelte:head>
	<title>GitPrompt</title>
</svelte:head>

<a class="skip" href="#main">Skip to content</a>

{#if navigating.to}<div class="progress" aria-hidden="true"></div>{/if}

<div class="shell">
	<Sidebar />
	<main id="main" tabindex="-1" class={[navigating.to && 'loading']}>
		{@render children()}
	</main>
</div>

<style>
	.shell {
		display: grid;
		grid-template-columns: var(--sidebar-width) minmax(0, 1fr);
		min-height: 100dvh;
	}

	main {
		width: 100%;
		max-width: var(--content-max);
		padding: var(--space-8) var(--space-8) var(--space-16);
		transition: opacity var(--duration-base) var(--ease-out);
	}

	main:focus-visible {
		box-shadow: none;
	}

	/* Refetch keeps the frame: the old content stays, dimmed, until the new one arrives. */
	main.loading {
		opacity: 0.6;
		transition-delay: 150ms;
	}

	.skip {
		position: absolute;
		top: var(--space-2);
		left: var(--space-2);
		z-index: var(--z-toast);
		padding: var(--space-2) var(--space-3);
		border-radius: var(--radius-md);
		background: var(--color-surface-raised);
		box-shadow: var(--shadow-overlay);
		transform: translateY(-200%);
	}

	.skip:focus-visible {
		transform: none;
	}

	.progress {
		position: fixed;
		top: 0;
		left: 0;
		z-index: var(--z-toast);
		width: 100%;
		height: 2px;
		background: var(--color-accent);
		transform-origin: left;
		animation: progress 1.6s var(--ease-out) forwards;
	}

	@keyframes progress {
		from {
			transform: scaleX(0);
		}
		to {
			transform: scaleX(0.85);
		}
	}

	@media (max-width: 56rem) {
		.shell {
			grid-template-columns: minmax(0, 1fr);
			grid-template-rows: auto 1fr;
		}

		main {
			padding: var(--space-6) var(--space-4) var(--space-12);
		}
	}
</style>
