<script lang="ts">
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import Button from '#lib/ui/Button.svelte';
	import CodeBlock from '#lib/ui/CodeBlock.svelte';
</script>

<svelte:head><title>{page.status} · GitPrompt</title></svelte:head>

<div class="error">
	<p class="status num">{page.status}</p>
	<h1>{page.status === 404 ? 'This page does not exist' : 'The dashboard could not load this page'}</h1>
	{#if page.error?.message}
		<CodeBlock code={page.error.message} label="Error" />
	{/if}
	{#if page.status !== 404}
		<p class="muted">
			If this appeared after an update, the daemon may be running older code than the dashboard:
			restart it with <code>just serve</code>. Details are in the browser console.
		</p>
	{/if}
	<div><Button variant="primary" href={resolve('/')}>Go to the overview</Button></div>
</div>

<style>
	.error {
		display: grid;
		gap: var(--space-4);
		max-width: 40rem;
		padding-top: var(--space-12);
	}

	.status {
		color: var(--color-accent-text);
		font-size: var(--text-5xl);
		font-weight: var(--weight-bold);
		font-stretch: 85%;
		line-height: var(--leading-none);
	}
</style>
