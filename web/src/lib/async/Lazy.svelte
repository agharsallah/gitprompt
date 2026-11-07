<script lang="ts" generics="Props extends Record<string, any>">
	import type { Component, Snippet } from 'svelte';
	import Loading from '#lib/async/Loading.svelte';
	import Boundary from '#lib/errors/Boundary.svelte';
	import ErrorView from '#lib/errors/ErrorView.svelte';

	/**
	 * A component fetched only when it is needed: its code is a separate file, imported when this
	 * renders (`when="now"`) or when it comes near the viewport (`when="visible"`). The skeleton in
	 * `pending` holds its place meanwhile. The import must be the component's only one, or the
	 * bundler keeps it in the page's own file:
	 *
	 *     <Lazy load={() => import('#lib/x/Heavy.svelte')} props={{ data }} label="Loading …" what="…">
	 *         {#snippet pending()}<PanelSkeleton />{/snippet}
	 *     </Lazy>
	 */
	let {
		load,
		props,
		when = 'now',
		label,
		what,
		pending
	}: {
		load: () => Promise<{ default: Component<Props, any, any> }>;
		props: Props;
		when?: 'now' | 'visible';
		label: string;
		/** What this is, for error titles: "this panel". */
		what: string;
		pending: Snippet;
	} = $props();

	let Loaded = $state.raw<Component<Props, any, any> | null>(null);
	let error = $state.raw<unknown>(null);
	let attempt = $state(0);
	let near = $state(false);
	let placeholder = $state<HTMLElement>();
	const wanted = $derived(when === 'now' || near);

	// Start the import once the placeholder is within a screen of the viewport.
	$effect(() => {
		if (wanted || !placeholder) return;
		const observer = new IntersectionObserver(
			(entries) => {
				if (entries.some((e) => e.isIntersecting)) near = true;
			},
			{ rootMargin: '100% 0px' }
		);
		observer.observe(placeholder);
		return () => observer.disconnect();
	});

	$effect(() => {
		if (!wanted || Loaded) return;
		void attempt;
		let current = true;
		load().then(
			(module) => current && (Loaded = module.default),
			(e) => {
				if (!current) return;
				console.error(`GitPrompt: ${what} could not be loaded`, e);
				error = e;
			}
		);
		return () => {
			current = false;
		};
	});

	function retry() {
		error = null;
		attempt++;
	}
</script>

{#if Loaded}
	<Boundary {what}><Loaded {...props} /></Boundary>
{:else if error}
	<ErrorView {error} {what} onretry={retry} />
{:else}
	<div bind:this={placeholder}>
		<Loading {label}>{@render pending()}</Loading>
	</div>
{/if}
