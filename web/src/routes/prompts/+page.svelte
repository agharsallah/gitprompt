<script lang="ts">
	import { ArrowLeft } from '@lucide/svelte';
	import { refreshAll } from '$app/navigation';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import { api } from '#lib/api/client.ts';
	import { liveUpdates } from '#lib/api/live.svelte.ts';
	import type { PromptDetail as Detail, Origin, Prompt } from '#lib/api/types.ts';
	import { isAll } from '#lib/prompts/originFilter.svelte.ts';
	import { outcome, supersededIds } from '#lib/insights/outcome.ts';
	import { eventTime } from '#lib/insights/timeline.ts';
	import { clock } from '#lib/shared/clock.svelte.ts';
	import { plural } from '#lib/shared/format.ts';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import PageHeader from '#lib/ui/PageHeader.svelte';
	import Spinner from '#lib/ui/Spinner.svelte';
	import PromptDetail from '#lib/prompts/PromptDetail.svelte';
	import PromptFilters, {
		isQuick,
		PREDICATES,
		QUICK,
		type Quick
	} from '#lib/prompts/PromptFilters.svelte';
	import PromptList from '#lib/prompts/PromptList.svelte';
	import { MAX_LIMIT, PAGE } from '#lib/prompts/paging.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	const selectedId = $derived(Number(page.url.searchParams.get('id')) || null);
	const show = $derived(page.url.searchParams.getAll('show').filter(isQuick));
	const shown = $derived(data.prompts.filter((p) => show.every((q) => PREDICATES[q](p))));
	const counts = $derived(
		Object.fromEntries(
			(Object.keys(QUICK) as Quick[]).map((q) => [q, data.prompts.filter(PREDICATES[q]).length])
		) as Record<Quick, number>
	);
	const superseded = $derived(supersededIds(data.prompts));
	// Prompts the server-side filters keep over every page (counts from /api/stats): of any
	// origin, and of the origins shown.
	const counted = (keep: (o: Origin) => boolean) =>
		Object.entries(data.originCounts)
			.filter(([o]) => keep(o as Origin))
			.reduce((n, [, count]) => n + (count ?? 0), 0);
	const anyOrigin = $derived(counted(() => true));
	const matching = $derived(counted((o) => isAll(data.origins) || data.origins.includes(o)));
	// With no prompt of any origin and no other filter, this is a first run, not a filter miss.
	const filtered = $derived(
		Object.keys(data.filters).length > 0 ||
			show.length > 0 ||
			(!isAll(data.origins) && anyOrigin > 0)
	);

	/** This page's URL with some parameters changed (null removes one). */
	function url(changes: Record<string, string | null>) {
		const params = new URLSearchParams(page.url.search);
		for (const [key, value] of Object.entries(changes)) {
			if (value === null) params.delete(key);
			else params.set(key, value);
		}
		const query = params.toString();
		return `${resolve('/prompts')}${query ? `?${query}` : ''}`;
	}
	const hrefFor = (p: Prompt) => url({ id: String(p.id) });

	// The selected prompt's detail: fetched on selection, refetched on live updates.
	let detail = $state.raw<Detail | null>(null);
	let detailError = $state('');
	let version = $state(0);
	$effect(() => {
		const id = selectedId;
		void version;
		if (!id) {
			detail = null;
			return;
		}
		let current = true;
		api
			.prompt(id)
			.then((d) => {
				if (!current) return;
				detail = d;
				detailError = '';
			})
			.catch((e) => current && (detailError = e instanceof Error ? e.message : String(e)));
		return () => (current = false);
	});
	const loadingDetail = $derived(!!selectedId && detail?.id !== selectedId && !detailError);

	const lastEvent = (d: Detail) =>
		d.events.length ? Math.max(...d.events.map(eventTime)) : undefined;

	const reload = async () => {
		await refreshAll();
		version++;
	};
	$effect(() => liveUpdates(reload));

	const full = $derived(data.prompts.length >= data.limit);
	const last = $derived(data.prompts.at(-1));
</script>

<svelte:head><title>Prompts · GitPrompt</title></svelte:head>

<div class={['page', selectedId && 'has-detail']}>
	<div class="list-column">
		<PageHeader
			title="Prompts"
			description="Everything you asked, newest first. Prompts of one session share a line; select one to see how the turn went."
		/>
		<PromptFilters
			filters={data.filters}
			{show}
			{counts}
			origins={data.origins}
			originCounts={data.originCounts}
		/>

		<p class="summary muted" aria-live="polite">
			{#if show.length}
				{plural(shown.length, 'prompt')} match, among the {data.prompts.length} loaded
			{:else if data.filters.before || data.prompts.length >= matching}
				{plural(data.prompts.length, 'prompt')}{data.filters.before ? ', older ones' : ''}
			{:else}
				Newest {data.prompts.length} of {plural(matching, 'prompt')}
			{/if}
		</p>

		<div class="list">
			{#if shown.length}
				<PromptList prompts={shown} {selectedId} {hrefFor} />
			{:else if filtered}
				<EmptyState title="No prompts match these filters">
					Try fewer words, another tag or more origins, or
					<a href={resolve('/prompts')}>clear all filters</a>.
				</EmptyState>
			{:else}
				<EmptyState title="No prompts yet">
					Connect a source, then type a prompt in it: it shows up here as soon as you send it.
					{#snippet actions()}
						<Button variant="primary" href={resolve('/sources')}>Connect a source</Button>
					{/snippet}
				</EmptyState>
			{/if}
		</div>

		{#if full || data.filters.before}
			<nav class="pager" aria-label="More prompts">
				{#if data.filters.before}
					<Button href={url({ before: null, limit: null, id: null })} size="sm"
						>Newest prompts</Button
					>
				{/if}
				{#if full && data.limit < MAX_LIMIT}
					<Button
						href={url({ limit: String(data.limit + PAGE) })}
						size="sm"
						data-sveltekit-reset="false"
					>
						Show {PAGE} more
					</Button>
				{:else if full && last}
					<Button href={url({ before: String(last.id), limit: null, id: null })} size="sm"
						>Older prompts</Button
					>
				{/if}
			</nav>
		{/if}
	</div>

	<div class="detail-column" aria-live="polite" aria-busy={loadingDetail}>
		{#if selectedId}
			<a class="back" href={url({ id: null })} data-sveltekit-reset="false"
				><ArrowLeft size={14} aria-hidden="true" />All prompts</a
			>
		{/if}
		{#if detailError}
			<Callout tone="danger" title="This prompt could not be loaded">{detailError}</Callout>
		{:else if detail}
			<div class={['detail', loadingDetail && 'stale']}>
				<PromptDetail
					prompt={detail}
					outcome={outcome(detail, clock.now, superseded.has(detail.id), lastEvent(detail))}
					closeHref={url({ id: null })}
					onchange={reload}
				/>
			</div>
		{:else if loadingDetail}
			<div class="loading"><Spinner size={18} /> Loading the prompt</div>
		{:else if shown.length}
			<div class="placeholder">
				<EmptyState title="Select a prompt">
					See what you asked, how long the agent worked, which tools and files it touched, what it
					cost, and what was committed afterwards.
				</EmptyState>
			</div>
		{/if}
	</div>
</div>

<style>
	.page {
		display: grid;
		grid-template-columns: minmax(20rem, 30rem) minmax(0, 1fr);
		gap: var(--space-8);
		align-items: start;
	}

	.list-column {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		gap: var(--space-4);
		min-width: 0;
	}

	.list-column :global(.page-header) {
		margin-bottom: 0;
	}

	.summary {
		font-size: var(--text-sm);
	}

	.list {
		border-radius: var(--radius-lg);
		background: var(--color-surface);
		box-shadow: inset 0 0 0 1px var(--color-border);
		padding: var(--space-1);
	}

	.pager {
		display: flex;
		gap: var(--space-2);
		justify-content: center;
	}

	.detail-column {
		position: sticky;
		top: var(--space-4);
		max-height: calc(100dvh - var(--space-8));
		min-width: 0;
		overflow-y: auto;
		padding: var(--space-6);
		border-radius: var(--radius-xl);
		background: var(--color-surface);
		box-shadow: inset 0 0 0 1px var(--color-border);
	}

	.detail-column:empty {
		display: none;
	}

	.detail {
		transition: opacity var(--duration-base) var(--ease-out);
	}

	.stale {
		opacity: 0.55;
	}

	.loading {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-8);
		color: var(--color-text-secondary);
	}

	.placeholder {
		display: grid;
		min-height: 24rem;
		place-items: center;
	}

	.back {
		display: none;
	}

	@media (max-width: 72rem) {
		.page {
			grid-template-columns: minmax(0, 1fr);
		}

		.has-detail .list-column {
			display: none;
		}

		.page:not(.has-detail) .detail-column {
			display: none;
		}

		.detail-column {
			position: static;
			max-height: none;
			padding: var(--space-4);
		}

		.back {
			display: inline-flex;
			align-items: center;
			gap: var(--space-1);
			margin-bottom: var(--space-4);
			font-size: var(--text-sm);
		}
	}
</style>
