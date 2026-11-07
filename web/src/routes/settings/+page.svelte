<script lang="ts">
	import Deferred from '#lib/async/Deferred.svelte';
	import { Resource } from '#lib/async/resource.svelte.ts';
	import Boundary from '#lib/errors/Boundary.svelte';
	import BrowserSection from '#lib/settings/BrowserSection.svelte';
	import CaptureSection from '#lib/settings/CaptureSection.svelte';
	import ConnectorsSection from '#lib/settings/ConnectorsSection.svelte';
	import DataSection from '#lib/settings/DataSection.svelte';
	import GeneralSection from '#lib/settings/GeneralSection.svelte';
	import PrivacySection from '#lib/settings/PrivacySection.svelte';
	import SecuritySection from '#lib/settings/SecuritySection.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import PageHeader from '#lib/ui/PageHeader.svelte';
	import PanelSkeleton from '#lib/ui/PanelSkeleton.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	const content = new Resource(() => data.content);
	const settings = $derived(content.value?.settings);
	const prefs = $derived(settings && !('error' in settings) ? settings : null);
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

<!-- Each section fails on its own: a broken one leaves the others usable. -->
<Deferred resource={content} label="Loading settings" what="your settings">
	{#snippet pending()}
		<div class="sections">
			<PanelSkeleton body="rows" rows={3} />
			<PanelSkeleton body="rows" rows={4} />
			<PanelSkeleton body="rows" rows={3} />
		</div>
	{/snippet}
	{#snippet children(c)}
		{#if 'error' in c.settings}
			<Callout tone="danger" title="Your preferences can't be read">
				{c.settings.error}. GitPrompt uses the defaults until the file is fixed or deleted.
			</Callout>
		{/if}

		<div class="sections">
			{#if prefs}
				<div id="general">
					<Boundary what="the general settings"><GeneralSection {prefs} /></Boundary>
				</div>
			{/if}
			<div id="connectors">
				<Boundary what="the connector settings">
					<ConnectorsSection sources={c.sources} problems={c.problems} />
				</Boundary>
			</div>
			{#if prefs}
				<div id="capture">
					<Boundary what="the capture settings"><CaptureSection {prefs} /></Boundary>
				</div>
				<div id="privacy">
					<Boundary what="the privacy settings"><PrivacySection {prefs} /></Boundary>
				</div>
			{/if}
			<div id="data">
				<Boundary what="the data settings">
					<DataSection
						data={c.data}
						retentionDays={prefs?.settings.event_retention_days.value ?? null}
					/>
				</Boundary>
			</div>
			<div id="security">
				<Boundary what="the security settings"><SecuritySection security={c.security} /></Boundary>
			</div>
			<div id="browser">
				<Boundary what="the browser settings"><BrowserSection /></Boundary>
			</div>
		</div>
	{/snippet}
</Deferred>

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
