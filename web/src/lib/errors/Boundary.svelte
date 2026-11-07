<script lang="ts">
	import type { Snippet } from 'svelte';
	import ErrorView from '#lib/errors/ErrorView.svelte';

	/**
	 * Keeps a crash where it happened: if anything inside throws while rendering (or in an effect),
	 * only this part is replaced by the error and a "Try again" that renders it afresh. The rest of
	 * the page, and the sidebar, keep working. Errors in event handlers are not caught here: actions
	 * report their own (lib/shared/task.svelte.ts).
	 */
	let {
		what,
		variant = 'block',
		children
	}: {
		/** What this part is, for the message: "this panel", "the session story". */
		what: string;
		variant?: 'page' | 'block';
		children: Snippet;
	} = $props();
</script>

<svelte:boundary onerror={(error) => console.error(`GitPrompt: ${what} failed to render`, error)}>
	{@render children()}

	{#snippet failed(error, reset)}
		<ErrorView {error} {what} {variant} onretry={reset} />
	{/snippet}
</svelte:boundary>
