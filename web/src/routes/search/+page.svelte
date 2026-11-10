<script lang="ts">
	import { Search, Tag } from '@lucide/svelte';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import Deferred from '#lib/async/Deferred.svelte';
	import { Resource } from '#lib/async/resource.svelte.ts';
	import RangePicker from '#lib/insights/RangePicker.svelte';
	import { rangePhrase } from '#lib/insights/range.ts';
	import OriginChips from '#lib/prompts/OriginChips.svelte';
	import { folder, integer, plural } from '#lib/shared/format.ts';
	import SearchHit from '#lib/stories/SearchHit.svelte';
	import Button from '#lib/ui/Button.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import ListSkeleton from '#lib/ui/ListSkeleton.svelte';
	import PageHeader from '#lib/ui/PageHeader.svelte';
	import Select from '#lib/ui/Select.svelte';
	import TextField from '#lib/ui/TextField.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	let form: HTMLFormElement;
	const submit = () => form.requestSubmit();

	const results = new Resource(() => data.results);
	// The form is usable at once; project choices and origin counts fill in when they arrive.
	const facets = new Resource(() => data.facets);
	const projects = $derived(facets.value?.projects ?? []);
	const range = $derived(page.url.searchParams.get('range'));
	function moreHref() {
		const params = new URLSearchParams(page.url.search);
		params.set('limit', String(data.limit + data.page));
		return `${resolve('/search')}?${params}`;
	}
</script>

<svelte:head><title>{data.q ? `${data.q} · ` : ''}Search · GitPrompt</title></svelte:head>

<PageHeader
	title="Search"
	description="Find what you asked, word for word: every word must appear in the text you typed. Each result opens the prompt or its place in a story."
>
	{#snippet actions()}<RangePicker range={data.filters.range} />{/snippet}
</PageHeader>

<form
	bind:this={form}
	class="form"
	method="GET"
	action={resolve('/search')}
	role="search"
	data-sveltekit-keepfocus
>
	{#if range}<input type="hidden" name="range" value={range} />{/if}
	<div class="row">
		<div class="q">
			<TextField
				label="Words to find"
				hideLabel
				type="search"
				name="q"
				icon={Search}
				value={data.q}
				placeholder="Words from a prompt"
				clearable
			/>
		</div>
		<Select
			label="Project"
			hideLabel
			name="repo"
			anyLabel="All projects"
			value={data.filters.repo ?? ''}
			options={projects.map((p) => ({
				value: p.key!,
				label: p.detail ?? folder(p.key) ?? p.key!
			}))}
			onchange={submit}
		/>
		<div class="tag">
			<TextField
				label="Tag"
				hideLabel
				name="tag"
				icon={Tag}
				value={data.tag ?? ''}
				placeholder="Tag"
			/>
		</div>
		<Button type="submit" variant="primary">Search</Button>
	</div>
	<OriginChips
		origins={data.filters.origins}
		counts={facets.value?.originCounts ?? {}}
		onchange={submit}
	/>
</form>

{#if !data.q}
	<EmptyState title="Type a few words">
		Search covers the text of every prompt, without the context the tool added. Short words work
		too.
	</EmptyState>
{:else}
	<Deferred resource={results} label="Searching" what="the search results">
		{#snippet pending()}<ListSkeleton variant="rows" rows={6} />{/snippet}
		{#snippet children(r)}
			{#if r && r.hits.length}
				<p class="summary muted" aria-live="polite">
					{r.total > r.hits.length
						? `Newest ${integer(r.hits.length)} of ${plural(r.total, 'prompt')}`
						: plural(r.total, 'prompt')}
					{rangePhrase(data.filters.range)}
				</p>
				<ol class="hits" aria-label="Search results">
					{#each r.hits as hit (hit.id)}<SearchHit {hit} />{/each}
				</ol>
				{#if r.hits.length < r.total && data.limit < 200}
					<nav class="pager" aria-label="More results">
						<Button href={moreHref()} size="sm" data-sveltekit-noscroll
							>Show {data.page} more</Button
						>
					</nav>
				{/if}
			{:else}
				<EmptyState title="No prompt has all these words">
					Try fewer words, another project, a longer range or more origins.
				</EmptyState>
			{/if}
		{/snippet}
	</Deferred>
{/if}

<style>
	.form {
		display: grid;
		gap: var(--space-3);
		margin-bottom: var(--space-5);
	}

	.row {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
	}

	.q {
		flex: 1 1 18rem;
		max-width: 32rem;
	}

	.tag {
		width: 9rem;
	}

	.summary {
		margin-bottom: var(--space-3);
		font-size: var(--text-sm);
	}

	.hits {
		border-radius: var(--radius-lg);
		background: var(--color-surface);
		box-shadow: inset 0 0 0 1px var(--color-border);
		padding: var(--space-1);
	}

	.pager {
		display: flex;
		justify-content: center;
		margin-top: var(--space-4);
	}
</style>
