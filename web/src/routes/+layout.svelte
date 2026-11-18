<script lang="ts">
	// Fonts are bundled, never fetched from a CDN: the dashboard makes no request off this machine.
	import '@fontsource-variable/instrument-sans/wdth.css';
	import '@fontsource/commit-mono/400.css';
	import '@fontsource/commit-mono/600.css';
	import '#lib/design/tokens.css';
	import '#lib/design/base.css';
	import { setLucideProps } from '@lucide/svelte';
	import { navigating, page } from '$app/state';
	import Lazy from '#lib/async/Lazy.svelte';
	import Boundary from '#lib/errors/Boundary.svelte';
	import Sidebar from '#lib/layout/Sidebar.svelte';
	// Every feature's palette commands register themselves when this is imported (one file each).
	import '#lib/palette/features.ts';
	import { isShortcut } from '#lib/palette/keys.ts';
	import { isRecall } from '#lib/palette/mode.ts';
	import { palette } from '#lib/palette/open.svelte.ts';
	import { isViewPage } from '#lib/settings/views.ts';
	import { views } from '#lib/settings/views.svelte.ts';
	import { forgetSeen } from '#lib/shared/seen.ts';
	import { track } from '#lib/shared/usage.ts';
	import { PageWatch } from '#lib/shared/usage-page.ts';
	import RecallHost from '#lib/palette/RecallHost.svelte';
	import ToastHost from '#lib/ui/ToastHost.svelte';
	import { toasts } from '#lib/ui/toast.svelte.ts';
	import { live } from '#lib/api/live.svelte.ts';
	import { showNotification } from '#lib/shared/notifications.ts';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();

	// The desktop recall window (`/?recall=1`) is the palette alone: no page, no sidebar, and what
	// it is opened at is not a visit to the Overview.
	const recall = $derived(isRecall(page.url));

	// Counted here, once, for every page: the page opened, and the filters changed on it.
	const watch = new PageWatch(forgetSeen);
	$effect(() => {
		if (recall) return;
		for (const action of watch.visit(page.route.id, page.url)) track(action);
	});

	// The one place a page's view is kept (lib/settings/views.ts): when the URL you are on changes,
	// never in a load (links preloaded on hover run loads too).
	$effect(() => {
		const route = page.route.id;
		if (!recall && isViewPage(route)) views.keep(route, page.url);
	});

	// What the daemon decided deserves a notification, as a toast while the dashboard is open
	// (not in the recall window, which is opened for one quick search).
	$effect(() => {
		if (recall) return;
		return live.onNotification((notification) => showNotification(toasts, notification));
	});

	// ⌘K (Ctrl K) opens the palette from any page, even from a text field.
	function onkeydown(event: KeyboardEvent) {
		if (recall || !isShortcut(event)) return;
		event.preventDefault();
		palette.toggle();
	}

	setLucideProps({ size: 16, strokeWidth: 1.75 });
</script>

<svelte:head>
	<title>GitPrompt</title>
</svelte:head>

<svelte:window {onkeydown} />

{#if recall}
	<RecallHost />
{:else}
	{@render dashboard()}
{/if}

{#snippet dashboard()}
	<a class="skip" href="#main">Skip to content</a>

	{#if navigating.to}<div class="progress" aria-hidden="true"></div>{/if}

	<div class="shell">
		<!-- Each part fails on its own: a crash in the page leaves the sidebar to navigate away, and the
	     page's boundary starts afresh on the next page. -->
		<div class="sidebar">
			<Boundary what="the navigation"><Sidebar /></Boundary>
		</div>
		<main id="main" tabindex="-1" class={[navigating.to && 'loading']}>
			{#key page.url.pathname}
				<Boundary what="this page" variant="page">{@render children()}</Boundary>
			{/key}
		</main>
	</div>

	<!-- The palette's code is fetched the first time it is opened, so pages pay nothing before. -->
	{#if palette.loaded}
		<Lazy
			load={() => import('#lib/palette/Palette.svelte')}
			props={{ palette }}
			label="Opening the palette"
			what="the palette"
		>
			{#snippet pending()}{/snippet}
		</Lazy>
	{/if}

	<ToastHost />
{/snippet}

<style>
	.shell {
		display: grid;
		grid-template-columns: var(--sidebar-width) minmax(0, 1fr);
		min-height: 100dvh;
	}

	.sidebar {
		display: contents;
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
