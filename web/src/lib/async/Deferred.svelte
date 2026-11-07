<script lang="ts" generics="T">
	import type { Snippet } from 'svelte';
	import { refreshAll } from '$app/navigation';
	import Loading from '#lib/async/Loading.svelte';
	import type { Resource } from '#lib/async/resource.svelte.ts';
	import Boundary from '#lib/errors/Boundary.svelte';
	import ErrorView from '#lib/errors/ErrorView.svelte';

	/**
	 * The part of a page that waits for streamed data (lib/async/resource.svelte.ts):
	 * - nothing yet: the `pending` skeleton;
	 * - the data: `children`, dimmed while other data (new filters) is on its way;
	 * - an error: in place of the content, with "Try again" (which runs the page's load again);
	 * - an error after data was shown: the error above the data it could not replace.
	 * A crash while rendering `children` stays inside this part too.
	 */
	let {
		resource,
		label,
		what,
		pending,
		children,
		variant = 'block',
		onretry = () => refreshAll()
	}: {
		resource: Resource<T>;
		/** Announced while loading: "Loading the overview". */
		label: string;
		/** What this is, for error titles: "the overview". */
		what: string;
		pending: Snippet;
		children: Snippet<[T]>;
		/** How an error with nothing to show is drawn: `page` when this is the page's whole content. */
		variant?: 'page' | 'block';
		onretry?: () => unknown;
	} = $props();
</script>

{#if resource.ready}
	{#if resource.failed && !resource.loading}
		<div class="refresh-error">
			<ErrorView
				error={resource.error}
				{what}
				note="What you see below was loaded earlier."
				{onretry}
			/>
		</div>
	{/if}
	<div class={['content', resource.changing && 'changing']} aria-busy={resource.loading}>
		<Boundary {what}>{@render children(resource.value as T)}</Boundary>
	</div>
{:else if resource.failed && !resource.loading}
	<ErrorView error={resource.error} {what} {variant} {onretry} />
{:else}
	<Loading {label}>{@render pending()}</Loading>
{/if}

<style>
	.refresh-error {
		margin-bottom: var(--space-4);
	}

	.content {
		transition: opacity var(--duration-base) var(--ease-out);
	}

	/* New filters: the old result stays, dimmed, until the new one arrives (after a beat, so a
	   fast answer doesn't flicker). */
	.changing {
		opacity: 0.6;
		transition-delay: 150ms;
	}
</style>
