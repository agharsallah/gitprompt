<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { HTMLAnchorAttributes } from 'svelte/elements';

	/**
	 * The one look of a link that opens another level (a session, a project, a list): accent text
	 * ending in "›". `title` says where it goes ("Session › Rust switch"), so hovering shows the
	 * target before the click.
	 */
	let {
		href,
		title,
		class: className,
		children,
		...rest
	}: Omit<HTMLAnchorAttributes, 'children' | 'title' | 'href'> & {
		href: string;
		/** The target, named as "Level › name". */
		title: string;
		children: Snippet;
	} = $props();
</script>

<a class={['link-arrow', className]} {href} {title} {...rest}
	>{@render children()}<span class="arrow" aria-hidden="true">›</span></a
>

<style>
	.link-arrow {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
		color: var(--color-accent-text);
		font-weight: var(--weight-medium);
		text-decoration: none;
	}

	.link-arrow:hover {
		color: var(--color-accent-hover);
		text-decoration: underline;
	}
</style>
