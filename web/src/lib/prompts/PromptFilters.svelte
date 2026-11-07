<script lang="ts" module>
	import type { Prompt } from '#lib/api/types.ts';

	/** Quick filters applied in the browser to the prompts already loaded (`?show=`). */
	export const QUICK = {
		failed: 'Failed',
		files: 'Changed files',
		commands: 'Ran commands',
		secrets: 'Possible secret'
	} as const;
	export type Quick = keyof typeof QUICK;

	export const PREDICATES: Record<Quick, (p: Prompt) => boolean> = {
		failed: (p) => p.summary.ended_by === 'StopFailure',
		files: (p) => !!p.summary.files_written?.length,
		commands: (p) => !!p.summary.bash_commands?.length,
		secrets: (p) => !!p.stats.secrets?.length
	};

	export const isQuick = (value: string): value is Quick => value in QUICK;

	/** The filters offered as selects on the Prompts page, in the order shown. */
	export const PROMPT_FACETS = ['source', 'surface', 'model', 'outcome', 'repo'] as const;
</script>

<script lang="ts">
	import { CircleX, FilePen, KeyRound, Search, SquareTerminal, Star, Tag } from '@lucide/svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import type { Filters, Origin, Stats } from '#lib/api/types.ts';
	import FacetSelects from '#lib/insights/FacetSelects.svelte';
	import type { Facet } from '#lib/insights/filters.ts';
	import OriginChips from '#lib/prompts/OriginChips.svelte';
	import { originFilter, parseOrigins } from '#lib/prompts/originFilter.svelte.ts';
	import { pins, unpin, type Lists, type Pin } from '#lib/prompts/pinned.ts';
	import Button from '#lib/ui/Button.svelte';
	import Chip from '#lib/ui/Chip.svelte';
	import TextField from '#lib/ui/TextField.svelte';
	import ToggleChip from '#lib/ui/ToggleChip.svelte';

	let {
		filters,
		lists,
		show,
		counts,
		origins,
		originCounts,
		facetOptions
	}: {
		filters: Filters;
		/** The multi-valued filters besides origin. */
		lists: Lists;
		show: Quick[];
		/** How many loaded prompts each quick filter keeps. */
		counts: Record<Quick, number>;
		/** The origins shown (none = every origin). */
		origins: Origin[];
		/** How many prompts of each origin the other filters keep (all pages). */
		originCounts: Partial<Record<Origin, number>>;
		/** The choices of each select, counted with the other filters (all pages). */
		facetOptions: Partial<Record<Facet, Stats>>;
	} = $props();

	// A select shows one value; a link that sets several (two outcomes) keeps them as chips.
	const single = (values: string[]) => (values.length <= 1 ? (values[0] ?? '') : null);
	const facetValues = $derived({
		source: filters.source ?? '',
		repo: filters.repo ?? '',
		surface: single(lists.surface),
		model: single(lists.model),
		outcome: single(lists.outcome)
	});
	const selects = $derived(PROMPT_FACETS.filter((f) => facetValues[f] !== null));
	const values = $derived(
		Object.fromEntries(selects.map((f) => [f, facetValues[f] ?? ''])) as Partial<
			Record<Facet, string>
		>
	);

	const ICONS = { failed: CircleX, files: FilePen, commands: SquareTerminal, secrets: KeyRound };
	let form: HTMLFormElement;

	/** The current URL without one pinned value (and without the selection). */
	function without(pin: Pin) {
		const query = unpin(page.url.search, pin);
		return `${resolve('/prompts')}${query ? `?${query}` : ''}`;
	}

	const pinned = $derived(
		pins(filters, lists).filter((p) => !(selects as readonly string[]).includes(p.key))
	);
	const active = $derived(
		!!(
			filters.q ||
			filters.tag ||
			filters.starred ||
			pinned.length ||
			show.length ||
			selects.some((f) => values[f])
		)
	);
	const submit = () => form.requestSubmit();

	/** Remember the new choice for every prompt view, then load it. */
	function changeOrigins() {
		originFilter.set(parseOrigins(new FormData(form).getAll('origin').map(String)));
		submit();
	}
</script>

<!-- A plain GET form: filters live in the URL, so the page's load function does the fetching. -->
<form
	method="GET"
	action={resolve('/prompts')}
	bind:this={form}
	data-sveltekit-reset="false"
	role="search"
	aria-label="Filter prompts"
>
	<div class="fields">
		<div class="search">
			<TextField
				label="Search prompts"
				hideLabel
				type="search"
				name="q"
				icon={Search}
				value={filters.q ?? ''}
				placeholder="Search prompt text"
				clearable
			/>
		</div>
		<div class="tag">
			<TextField
				label="Tag"
				hideLabel
				name="tag"
				icon={Tag}
				value={filters.tag ?? ''}
				placeholder="Tag"
			/>
		</div>
		<Button type="submit" variant="secondary">Search</Button>
	</div>

	<FacetSelects facets={selects} {values} options={facetOptions} onchange={submit} />

	<OriginChips {origins} counts={originCounts} onchange={changeOrigins} />

	<div class="toggles">
		<ToggleChip
			name="starred"
			value="true"
			checked={!!filters.starred}
			icon={Star}
			onchange={submit}
		>
			Starred
		</ToggleChip>
		{#each Object.entries(QUICK) as [key, label] (key)}
			<ToggleChip
				name="show"
				value={key}
				checked={show.includes(key as Quick)}
				icon={ICONS[key as Quick]}
				count={counts[key as Quick]}
				onchange={submit}
			>
				{label}
			</ToggleChip>
		{/each}

		{#each pinned as p (`${p.key}=${p.value}`)}
			<input type="hidden" name={p.key} value={p.value} />
			<Chip
				onremove={() => goto(without(p), { reset: false })}
				removeLabel="Remove filter {p.label}"
			>
				{p.label}
			</Chip>
		{/each}

		{#if active}<a class="clear" href={resolve('/prompts')}>Clear all</a>{/if}
	</div>
</form>

<style>
	form {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		gap: var(--space-3);
	}

	.fields {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(6rem, 8rem) auto;
		gap: var(--space-2);
	}

	/* Grid items default to min-width: auto, which lets an input's intrinsic width overflow. */
	.fields > div {
		min-width: 0;
	}

	.toggles {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
	}

	.clear {
		margin-left: var(--space-1);
		font-size: var(--text-sm);
	}
</style>
