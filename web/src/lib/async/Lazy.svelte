<script lang="ts" generics="Props extends Record<string, unknown>">
	import type { Component, Snippet } from 'svelte';
	import Loading from '#lib/async/Loading.svelte';
	import Boundary from '#lib/errors/Boundary.svelte';
	import ErrorView from '#lib/errors/ErrorView.svelte';

	/**
	 * A component fetched only when it is needed: its code is a separate file, imported when this
	 * renders. The skeleton in `pending` holds its place meanwhile. The import must be the component's only one, or the
	 * bundler keeps it in the page's own file:
	 *
	 *     <Lazy load={() => import('#lib/x/Heavy.svelte')} props={{ data }} label="Loading …" what="…">
	 *         {#snippet pending()}<PanelSkeleton />{/snippet}
	 *     </Lazy>
	 */
	let {
		load,
		props,
		label,
		what,
		pending
	}: {
		load: () => Promise<{ default: Component<Props> }>;
		props: Props;
		label: string;
		/** What this is, for error titles: "this panel". */
		what: string;
		pending: Snippet;
	} = $props();

	let Loaded = $state.raw<Component<Props> | null>(null);
	let error = $state.raw<unknown>(null);
	let attempt = $state(0);

	$effect(() => {
		if (Loaded) return;
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
	<Loading {label}>{@render pending()}</Loading>
{/if}
