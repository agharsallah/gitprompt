<script lang="ts">
	import { ArrowLeft } from '@lucide/svelte';
	import { refreshAll } from '$app/navigation';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import { api } from '#lib/api/client.ts';
	import { liveUpdates } from '#lib/api/live.svelte.ts';
	import type { PromptDetail as Detail, Origin, Prompt } from '#lib/api/types.ts';
	import Deferred from '#lib/async/Deferred.svelte';
	import Lazy from '#lib/async/Lazy.svelte';
	import Loading from '#lib/async/Loading.svelte';
	import { Resource } from '#lib/async/resource.svelte.ts';
	import ErrorView from '#lib/errors/ErrorView.svelte';
	import { isAll } from '#lib/prompts/originFilter.svelte.ts';
	import { outcome, supersededIds } from '#lib/insights/outcome.ts';
	import { eventTime } from '#lib/insights/timeline.ts';
	import { clock } from '#lib/shared/clock.svelte.ts';
	import { plural } from '#lib/shared/format.ts';
	import ExportButton from '#lib/export/ExportButton.svelte';
	import RangePicker from '#lib/insights/RangePicker.svelte';
	import Button from '#lib/ui/Button.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import ListSkeleton from '#lib/ui/ListSkeleton.svelte';
	import PageHeader from '#lib/ui/PageHeader.svelte';
	import Skeleton from '#lib/ui/Skeleton.svelte';
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

	// The prompts stream in; the empty defaults are never shown (Deferred waits for the first list).
	const listing = new Resource(() => data.listing);
	const prompts = $derived(listing.value?.prompts ?? []);
	const originCounts = $derived(listing.value?.originCounts ?? {});
	const facetOptions = $derived(listing.value?.facetOptions ?? {});

	const selectedId = $derived(Number(page.url.searchParams.get('id')) || null);
	const show = $derived(page.url.searchParams.getAll('show').filter(isQuick));
	const shown = $derived(prompts.filter((p) => show.every((q) => PREDICATES[q](p))));
	const counts = $derived(
		Object.fromEntries(
			(Object.keys(QUICK) as Quick[]).map((q) => [q, prompts.filter(PREDICATES[q]).length])
		) as Record<Quick, number>
	);
	const superseded = $derived(supersededIds(prompts));
	// Prompts the server-side filters keep over every page (counts from /api/stats): of any
	// origin, and of the origins shown.
	const counted = (keep: (o: Origin) => boolean) =>
		Object.entries(originCounts)
			.filter(([o]) => keep(o as Origin))
			.reduce((n, [, count]) => n + (count ?? 0), 0);
	const anyOrigin = $derived(counted(() => true));
	const matching = $derived(counted((o) => isAll(data.origins) || data.origins.includes(o)));
	// With no prompt of any origin and no other filter, this is a first run, not a filter miss.
	const filtered = $derived(
		Object.keys(data.filters).length > 0 ||
			Object.values(data.lists).some((l) => l.length > 0) ||
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
	/** Every filter cleared; the time range stays, as on the other pages. */
	const cleared = $derived(
		`${resolve('/prompts')}${page.url.searchParams.has('range') ? `?range=${data.range.id}` : ''}`
	);

	// The selected prompt's detail: fetched on selection, refetched on live updates.
	let detail = $state.raw<Detail | null>(null);
	let detailError = $state.raw<unknown>(null);
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
				detailError = null;
			})
			.catch((e) => current && (detailError = e));
		return () => (current = false);
	});
	const loadingDetail = $derived(!!selectedId && detail?.id !== selectedId && !detailError);
	const retryDetail = () => {
		detailError = null;
		version++;
	};

	const lastEvent = (d: Detail) =>
		d.events.length ? Math.max(...d.events.map(eventTime)) : undefined;

	const reload = async () => {
		await refreshAll();
		version++;
	};
	$effect(() => liveUpdates(reload));

	const full = $derived(prompts.length >= data.limit);
	const last = $derived(prompts.at(-1));
</script>

<svelte:head><title>Prompts · GitPrompt</title></svelte:head>

{#snippet detailSkeleton()}
	<div class="detail-skeleton">
		<div class="detail-head">
			<Skeleton width="5rem" height="1.25rem" shape="block" />
			<Skeleton width="7rem" height="0.8rem" />
		</div>
		<Skeleton lines={4} height="1rem" />
		<Skeleton shape="block" height="7rem" />
		<div class="detail-head">
			{#each ['5rem', '6rem', '4.5rem', '5.5rem'] as width, i (i)}
				<Skeleton {width} height="1.75rem" shape="block" />
			{/each}
		</div>
		<Skeleton lines={3} />
	</div>
{/snippet}

<div class={['page', selectedId && 'has-detail']}>
	<div class="list-column">
		<PageHeader
			title="Prompts"
			description="Everything you asked, newest first. Prompts of one session share a line; select one to see how the turn went."
		>
			{#snippet actions()}
				<RangePicker range={data.range} />
				<ExportButton
					target={{
						kind: 'prompts',
						query: { ...data.query, ...data.lists, origin: data.origins }
					}}
				/>
			{/snippet}
		</PageHeader>
		<Deferred resource={listing} label="Loading prompts" what="the prompts">
			{#snippet pending()}
				<div class="listing">
					<div class="filter-skeleton">
						<Skeleton height="2.25rem" shape="block" />
						<Skeleton width="70%" height="1.5rem" shape="block" />
					</div>
					<Skeleton width="9rem" height="0.8rem" />
					<ListSkeleton variant="rows" rows={8} />
				</div>
			{/snippet}
			<div class="listing">
				<PromptFilters
					range={page.url.searchParams.get('range')}
					{cleared}
					filters={data.filters}
					lists={data.lists}
					{show}
					{counts}
					origins={data.origins}
					{originCounts}
					{facetOptions}
				/>

				<p class="summary muted" aria-live="polite">
					{#if show.length}
						{plural(shown.length, 'prompt')} match, among the {prompts.length} loaded
					{:else if data.filters.before || prompts.length >= matching}
						{plural(prompts.length, 'prompt')}{data.filters.before ? ', older ones' : ''}
					{:else}
						Newest {prompts.length} of {plural(matching, 'prompt')}
					{/if}
				</p>

				<div class="list">
					{#if shown.length}
						<PromptList prompts={shown} {selectedId} {hrefFor} />
					{:else if filtered}
						<EmptyState title="No prompts match these filters">
							Try fewer words, another tool, model or outcome, more origins, or
							<a href={cleared}>clear all filters</a>.
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
		</Deferred>
	</div>

	<div
		class={['detail-column', !selectedId && !shown.length && 'idle']}
		aria-live="polite"
		aria-busy={loadingDetail}
	>
		{#if selectedId}
			<a class="back" href={url({ id: null })} data-sveltekit-reset="false"
				><ArrowLeft size={14} aria-hidden="true" />All prompts</a
			>
		{/if}
		{#if detailError}
			<ErrorView error={detailError} what="this prompt" onretry={retryDetail} />
		{:else if detail}
			<div class={['detail', loadingDetail && 'stale']}>
				<!-- The detail's code loads with the first prompt you open, not with the list. -->
				<Lazy
					load={() => import('#lib/prompts/PromptDetail.svelte')}
					props={{
						prompt: detail,
						outcome: outcome(detail, clock.now, superseded.has(detail.id), lastEvent(detail)),
						closeHref: url({ id: null }),
						onchange: reload
					}}
					label="Loading the prompt"
					what="this prompt"
					pending={detailSkeleton}
				/>
			</div>
		{:else if loadingDetail}
			<Loading label="Loading the prompt">{@render detailSkeleton()}</Loading>
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

	/* Nothing selected and nothing listed (yet): no empty frame next to the list. */
	.detail-column.idle {
		display: none;
	}

	.detail {
		transition: opacity var(--duration-base) var(--ease-out);
	}

	.stale {
		opacity: 0.55;
	}

	.listing {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		gap: var(--space-4);
	}

	.filter-skeleton {
		display: grid;
		gap: var(--space-2);
	}

	.detail-skeleton {
		display: grid;
		gap: var(--space-5);
	}

	.detail-head {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-3);
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
