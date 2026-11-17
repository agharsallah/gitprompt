<script lang="ts">
	import {
		Activity,
		FolderGit2,
		MessagesSquare,
		Palette,
		Plug,
		Search,
		Settings,
		ShieldCheck,
		TableProperties,
		TextQuote,
		type LucideIcon
	} from '@lucide/svelte';
	import { page } from '$app/state';
	import BrandMark from '#lib/layout/BrandMark.svelte';
	import LiveStatus from '#lib/layout/LiveStatus.svelte';
	import { isCurrent, NAV, tooltip, type NavLink } from '#lib/layout/nav.ts';
	import ThemeToggle from '#lib/layout/ThemeToggle.svelte';
	import UpdateHint from '#lib/layout/UpdateHint.svelte';
	import { updates } from '#lib/layout/updates.svelte.ts';
	import { isMac, shortcutLabel } from '#lib/palette/keys.ts';
	import { palette } from '#lib/palette/open.svelte.ts';
	import { pageHref } from '#lib/shared/links.ts';
	import { isViewPage } from '#lib/settings/views.ts';
	import { views } from '#lib/settings/views.svelte.ts';

	// Re-read on every navigation (an update done on Connectors clears the hint once you leave)
	// and when the window comes back into focus (GitPrompt may have been upgraded meanwhile).
	$effect(() => {
		void page.url.pathname;
		updates.refresh();
	});
	const onConnectors = $derived(page.route.id === '/sources');

	const ICONS: Partial<Record<NavLink['route'], LucideIcon>> = {
		'/': Activity,
		'/projects': FolderGit2,
		'/sessions': MessagesSquare,
		'/prompts': TextQuote,
		'/sources': Plug,
		'/fields': TableProperties,
		'/settings': Settings,
		'/design': Palette
	};

	/** A filtering page opens in the view you left it in (its kept query, lib/settings/views.ts). */
	const href = (route: NavLink['route']) =>
		pageHref(route) + (isViewPage(route) ? views.search[route] : '');

	const hint = shortcutLabel(isMac());
</script>

<svelte:window onfocus={() => updates.refresh()} />

<aside class="sidebar">
	<a class="brand" href={href('/')}>
		<BrandMark />
		<span class="wordmark">GitPrompt</span>
	</a>

	<!-- Find is the palette, not a page: a button, with the shortcut that does the same. -->
	<button
		type="button"
		class="find"
		onclick={() => palette.show()}
		aria-label="Find anything"
		aria-keyshortcuts="Control+K Meta+K"
		title="Find anything ({hint})"
	>
		<Search size={16} aria-hidden="true" />
		<span class="find-label">Find</span>
		<kbd>{hint}</kbd>
	</button>

	<nav aria-label="Main">
		{#each NAV as group (group.label)}
			<div class="group">
				<p class="group-label" id="nav-{group.label}">{group.label}</p>
				<ul aria-labelledby="nav-{group.label}">
					{#each group.links as link (link.route)}
						{@const Icon = ICONS[link.route]}
						<li>
							<a
								href={href(link.route)}
								title={tooltip(link)}
								aria-current={isCurrent(link, page.route.id) ? 'page' : undefined}
							>
								{#if Icon}<Icon size={16} aria-hidden="true" />{/if}
								<span class="text">
									<span class="label">{link.label}</span>
									{#if link.purpose}<span class="purpose">{link.purpose}</span>{/if}
								</span>
								{#if link.route === '/sources' && updates.list.length}
									<span class="update-dot" title="Update available">
										<span class="visually-hidden">(update available)</span>
									</span>
								{/if}
							</a>
						</li>
					{/each}
				</ul>
			</div>
		{/each}
	</nav>

	<div class="footer">
		<div class="update"><UpdateHint hidden={onConnectors} /></div>
		<LiveStatus />
		<p class="local">
			<ShieldCheck size={14} aria-hidden="true" />
			Stays on this machine
		</p>
		<ThemeToggle />
	</div>
</aside>

<style>
	.sidebar {
		position: sticky;
		top: 0;
		display: flex;
		flex-direction: column;
		gap: var(--space-6);
		height: 100dvh;
		padding: var(--space-4) var(--space-3);
		border-right: var(--border-hairline) solid var(--color-border);
		background: var(--color-surface);
		overflow-y: auto;
	}

	.brand {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-1) var(--space-2);
		color: var(--color-text);
		text-decoration: none;
	}

	.wordmark {
		font-size: var(--text-lg);
		font-weight: var(--weight-bold);
		font-stretch: 88%;
		letter-spacing: var(--tracking-tight);
	}

	.find {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		height: var(--control-md);
		padding: 0 var(--space-2);
		border-radius: var(--radius-md);
		background: var(--color-surface-raised);
		box-shadow: inset 0 0 0 1px var(--color-border);
		color: var(--color-text-secondary);
		font-weight: var(--weight-medium);
		text-align: left;
	}

	.find:hover {
		color: var(--color-text);
		box-shadow: inset 0 0 0 1px var(--color-border-control);
	}

	.find-label {
		flex: 1;
	}

	kbd {
		color: var(--color-text-muted);
		font-family: var(--font-mono);
		font-size: var(--text-xs);
	}

	nav {
		display: grid;
		gap: var(--space-5);
	}

	.group-label {
		padding: 0 var(--space-2) var(--space-1);
		color: var(--color-text-muted);
		font-size: var(--text-xs);
		font-weight: var(--weight-medium);
	}

	ul {
		display: grid;
		gap: 1px;
	}

	nav a {
		position: relative;
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-height: var(--control-md);
		padding: var(--space-1) var(--space-2);
		border-radius: var(--radius-md);
		color: var(--color-text-secondary);
		font-weight: var(--weight-medium);
		text-decoration: none;
	}

	.text {
		display: grid;
		min-width: 0;
	}

	.purpose {
		color: var(--color-text-muted);
		font-size: var(--text-xs);
		font-weight: var(--weight-regular);
		line-height: var(--leading-snug);
	}

	nav a :global(svg) {
		color: var(--color-text-muted);
	}

	nav a:hover {
		background: var(--color-surface-hover);
		color: var(--color-text);
	}

	/* The current page: a heavier label and a bar, so it reads without the colour. */
	nav a[aria-current='page'] {
		background: var(--color-accent-soft);
		color: var(--color-accent-text);
		font-weight: var(--weight-bold);
	}

	nav a[aria-current='page'] .purpose {
		color: var(--color-accent-text);
	}

	nav a[aria-current='page'] :global(svg) {
		color: var(--color-accent);
	}

	/* The current page sits on the rail: a short accent line at the edge. */
	nav a[aria-current='page']::before {
		content: '';
		position: absolute;
		top: 8px;
		bottom: 8px;
		left: calc(-1 * var(--space-3));
		width: 3px;
		border-radius: 0 3px 3px 0;
		background: var(--color-accent);
	}

	.update-dot {
		width: 7px;
		height: 7px;
		margin-left: auto;
		border-radius: 50%;
		background: var(--color-info-solid);
	}

	.visually-hidden {
		position: absolute;
		width: 1px;
		height: 1px;
		overflow: hidden;
		clip: rect(0, 0, 0, 0);
		white-space: nowrap;
	}

	.update:empty {
		display: none;
	}

	.footer {
		display: grid;
		gap: var(--space-3);
		margin-top: auto;
		padding: var(--space-3) var(--space-2) 0;
		border-top: var(--border-hairline) solid var(--color-border);
	}

	.local {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		color: var(--color-text-muted);
		font-size: var(--text-xs);
	}

	/* Narrow screens: a top bar with a scrolling row of links. */
	@media (max-width: 56rem) {
		.sidebar {
			z-index: var(--z-sticky);
			flex-direction: row;
			align-items: center;
			gap: var(--space-3);
			height: auto;
			padding: var(--space-2) var(--space-4);
			border-right: 0;
			border-bottom: var(--border-hairline) solid var(--color-border);
			overflow: visible;
		}

		.wordmark,
		.group-label,
		.purpose,
		.find-label,
		kbd,
		.local,
		.update,
		.footer :global(.live .text) {
			display: none;
		}

		nav {
			display: flex;
			flex: 1;
			gap: var(--space-1);
			min-width: 0;
			overflow-x: auto;
			scrollbar-width: none;
		}

		.group,
		ul {
			display: flex;
			gap: 1px;
		}

		nav a {
			white-space: nowrap;
		}

		/* In the top bar the bar sits under the link. */
		nav a[aria-current='page']::before {
			top: auto;
			right: var(--space-2);
			bottom: 0;
			left: var(--space-2);
			width: auto;
			height: 3px;
			border-radius: 3px 3px 0 0;
		}

		.find {
			width: var(--control-md);
			justify-content: center;
			padding: 0;
		}

		.footer {
			display: flex;
			align-items: center;
			margin: 0;
			padding: 0;
			border: 0;
		}
	}

	/* Phones: icons only, the label stays the link's accessible name (and its tooltip). */
	@media (max-width: 40rem) {
		nav a .text {
			position: absolute;
			width: 1px;
			height: 1px;
			overflow: hidden;
			clip: rect(0, 0, 0, 0);
			white-space: nowrap;
		}

		nav a {
			width: var(--control-md);
			justify-content: center;
			padding: 0;
		}

		.update-dot {
			position: absolute;
			top: 6px;
			right: 6px;
		}
	}
</style>
