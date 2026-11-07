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
</script>

<script lang="ts">
	import { CircleX, FilePen, KeyRound, Search, SquareTerminal, Star, Tag } from '@lucide/svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import type { Filters, Origin } from '#lib/api/types.ts';
	import { ORIGINS } from '#lib/prompts/origin.ts';
	import { ALL_ORIGINS, originFilter, parseOrigins } from '#lib/prompts/originFilter.svelte.ts';
	import { folder } from '#lib/shared/format.ts';
	import Button from '#lib/ui/Button.svelte';
	import Chip from '#lib/ui/Chip.svelte';
	import TextField from '#lib/ui/TextField.svelte';
	import ToggleChip from '#lib/ui/ToggleChip.svelte';

	let {
		filters,
		show,
		counts,
		origins,
		originCounts
	}: {
		filters: Filters;
		show: Quick[];
		/** How many loaded prompts each quick filter keeps. */
		counts: Record<Quick, number>;
		/** The origins shown (none = every origin). */
		origins: Origin[];
		/** How many prompts of each origin the other filters keep (all pages). */
		originCounts: Partial<Record<Origin, number>>;
	} = $props();

	const ICONS = { failed: CircleX, files: FilePen, commands: SquareTerminal, secrets: KeyRound };
	let form: HTMLFormElement;

	/** The current URL without one parameter (and without the selection). */
	function without(key: string) {
		const params = new URLSearchParams(page.url.search);
		params.delete(key);
		params.delete('id');
		const query = params.toString();
		return `${resolve('/prompts')}${query ? `?${query}` : ''}`;
	}

	const pinned = $derived(
		[
			filters.session && { key: 'session', label: `Session ${filters.session.slice(0, 8)}` },
			filters.repo && { key: 'repo', label: folder(filters.repo) ?? filters.repo },
			filters.before && { key: 'before', label: 'Older prompts' }
		].filter((p): p is { key: string; label: string } => !!p)
	);
	const active = $derived(
		!!(filters.q || filters.tag || filters.starred || pinned.length || show.length)
	);
	const submit = () => form.requestSubmit();

	// Every origin you have prompts of, and any selected one: yours first, then system ones.
	const shownOrigins = $derived(
		ALL_ORIGINS.filter((o) => origins.includes(o) || (originCounts[o] ?? 0) > 0)
	);
	const yours = $derived(shownOrigins.filter((o) => !ORIGINS[o].system));
	const system = $derived(shownOrigins.filter((o) => ORIGINS[o].system));

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

	<fieldset class="toggles origins">
		<legend class="sr-only">Who wrote the prompt</legend>
		{#snippet originChip(o: Origin)}
			<ToggleChip
				name="origin"
				value={o}
				checked={origins.includes(o)}
				icon={ORIGINS[o].icon}
				count={originCounts[o] ?? 0}
				title={ORIGINS[o].hint}
				onchange={changeOrigins}
			>
				{ORIGINS[o].label}
			</ToggleChip>
		{/snippet}
		{#each yours as o (o)}{@render originChip(o)}{/each}
		{#if system.length}
			<span class="group-label">System</span>
			{#each system as o (o)}{@render originChip(o)}{/each}
		{/if}
	</fieldset>

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

		{#each pinned as p (p.key)}
			<input type="hidden" name={p.key} value={filters[p.key as keyof Filters]} />
			<Chip
				onremove={() => goto(without(p.key), { reset: false })}
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

	fieldset {
		margin: 0;
		padding: 0;
		border: 0;
		min-width: 0;
	}

	.group-label {
		margin-left: var(--space-1);
		color: var(--color-text-muted);
		font-size: var(--text-xs);
		font-weight: var(--weight-medium);
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}

	.clear {
		margin-left: var(--space-1);
		font-size: var(--text-sm);
	}
</style>
