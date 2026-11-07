<script lang="ts">
	import { refreshAll } from '$app/navigation';
	import { page } from '$app/state';
	import ErrorView from '#lib/errors/ErrorView.svelte';
	import { describeError } from '#lib/errors/describe.ts';

	// Load errors land here (a missing route, a story that doesn't exist, a page whose code failed
	// to load). Data that streams in reports its errors in place instead (lib/async/Deferred.svelte).
	const title = $derived(describeError(page.error, page.status).title);
</script>

<svelte:head><title>{title} · GitPrompt</title></svelte:head>

<ErrorView
	error={page.error}
	status={page.status}
	variant="page"
	onretry={page.status === 404 ? undefined : () => refreshAll()}
/>
