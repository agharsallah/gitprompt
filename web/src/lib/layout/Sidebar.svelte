<script lang="ts">
	import {
		Activity,
		FolderGit2,
		MessagesSquare,
		Palette,
		Plug,
		Settings,
		ShieldCheck,
		TableProperties,
		TextQuote
	} from '@lucide/svelte';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import BrandMark from '#lib/layout/BrandMark.svelte';
	import LiveStatus from '#lib/layout/LiveStatus.svelte';
	import ThemeToggle from '#lib/layout/ThemeToggle.svelte';

	const groups = [
		{
			label: 'Review',
			links: [
				{ route: '/', label: 'Overview', icon: Activity },
				{ route: '/prompts', label: 'Prompts', icon: TextQuote },
				{ route: '/sessions', label: 'Sessions', icon: MessagesSquare },
				{ route: '/projects', label: 'Projects', icon: FolderGit2 }
			]
		},
		{
			label: 'Setup',
			links: [
				{ route: '/sources', label: 'Sources', icon: Plug },
				{ route: '/fields', label: 'Fields', icon: TableProperties },
				{ route: '/settings', label: 'Settings', icon: Settings },
				{ route: '/design', label: 'Design system', icon: Palette }
			]
		}
	] as const;
</script>

<aside class="sidebar">
	<a class="brand" href={resolve('/')}>
		<BrandMark />
		<span class="wordmark">GitPrompt</span>
	</a>

	<nav aria-label="Main">
		{#each groups as group (group.label)}
			<div class="group">
				<p class="group-label" id="nav-{group.label}">{group.label}</p>
				<ul aria-labelledby="nav-{group.label}">
					{#each group.links as link (link.route)}
						<li>
							<a
								href={resolve(link.route)}
								aria-current={page.route.id === link.route ? 'page' : undefined}
							>
								<link.icon size={16} aria-hidden="true" />
								<span>{link.label}</span>
							</a>
						</li>
					{/each}
				</ul>
			</div>
		{/each}
	</nav>

	<div class="footer">
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
		height: var(--control-md);
		padding: 0 var(--space-2);
		border-radius: var(--radius-md);
		color: var(--color-text-secondary);
		font-weight: var(--weight-medium);
		text-decoration: none;
	}

	nav a :global(svg) {
		color: var(--color-text-muted);
	}

	nav a:hover {
		background: var(--color-surface-hover);
		color: var(--color-text);
	}

	nav a[aria-current='page'] {
		background: var(--color-accent-soft);
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
		.local,
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

		nav a[aria-current='page']::before {
			display: none;
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
		nav a span {
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
	}
</style>
