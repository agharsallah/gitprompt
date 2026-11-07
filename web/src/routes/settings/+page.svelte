<script lang="ts">
	import BrowserSection from '#lib/settings/BrowserSection.svelte';
	import CaptureSection from '#lib/settings/CaptureSection.svelte';
	import ConnectorsSection from '#lib/settings/ConnectorsSection.svelte';
	import DataSection from '#lib/settings/DataSection.svelte';
	import GeneralSection from '#lib/settings/GeneralSection.svelte';
	import PrivacySection from '#lib/settings/PrivacySection.svelte';
	import SecuritySection from '#lib/settings/SecuritySection.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import PageHeader from '#lib/ui/PageHeader.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	const prefs = $derived('error' in data.settings ? null : data.settings);
	const SECTIONS = [
		['general', 'General'],
		['connectors', 'Connectors'],
		['capture', 'Capture'],
		['privacy', 'Privacy'],
		['data', 'Data'],
		['security', 'Security'],
		['browser', 'This browser']
	] as const;
</script>

<svelte:head><title>Settings · GitPrompt</title></svelte:head>

<PageHeader title="Settings" description="Everything you can configure, in one place.">
	{#if prefs}
		<p class="muted file">
			Saved in <span class="mono">{prefs.file}</span>. Environment variables (GITPROMPT_…) win over
			the file.
		</p>
	{/if}
</PageHeader>

<nav class="toc" aria-label="Settings sections">
	{#each SECTIONS as [id, label] (id)}<a href="#{id}">{label}</a>{/each}
</nav>

{#if 'error' in data.settings}
	<Callout tone="danger" title="Your preferences can't be read">
		{data.settings.error}. GitPrompt uses the defaults until the file is fixed or deleted.
	</Callout>
{/if}

<div class="sections">
	{#if prefs}<div id="general"><GeneralSection {prefs} /></div>{/if}
	<div id="connectors"><ConnectorsSection sources={data.sources} /></div>
	{#if prefs}
		<div id="capture"><CaptureSection {prefs} /></div>
		<div id="privacy"><PrivacySection {prefs} /></div>
	{/if}
	<div id="data">
		<DataSection
			data={data.data}
			retentionDays={prefs?.settings.event_retention_days.value ?? null}
		/>
	</div>
	<div id="security"><SecuritySection security={data.security} /></div>
	<div id="browser"><BrowserSection /></div>
</div>

<style>
	.file {
		font-size: var(--text-sm);
	}

	.file .mono {
		font-size: var(--text-xs);
		overflow-wrap: anywhere;
	}

	.toc {
		position: sticky;
		top: 0;
		z-index: var(--z-sticky);
		display: flex;
		gap: var(--space-1);
		margin: 0 calc(-1 * var(--space-2)) var(--space-4);
		padding: var(--space-2);
		background: var(--color-canvas);
		overflow-x: auto;
		scrollbar-width: none;
	}

	.toc a {
		padding: var(--space-1) var(--space-2);
		border-radius: var(--radius-md);
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
		font-weight: var(--weight-medium);
		text-decoration: none;
		white-space: nowrap;
	}

	.toc a:hover {
		background: var(--color-surface-hover);
		color: var(--color-text);
	}

	.sections {
		display: grid;
		gap: var(--space-6);
		max-width: 56rem;
	}

	.sections > div {
		scroll-margin-top: var(--space-12);
	}
</style>
