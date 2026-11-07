<script lang="ts">
	import { BookCheck, Eye, EyeOff, Search, Sparkles } from '@lucide/svelte';
	import type { Field } from '#lib/api/types.ts';
	import { inline, integer, percent, plural } from '#lib/shared/format.ts';
	import { CAPABILITIES, CAPABILITY_IDS } from '#lib/sources/capabilities.ts';
	import Badge from '#lib/ui/Badge.svelte';
	import Disclosure from '#lib/ui/Disclosure.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import PageHeader from '#lib/ui/PageHeader.svelte';
	import Panel from '#lib/ui/Panel.svelte';
	import SegmentedControl from '#lib/ui/SegmentedControl.svelte';
	import StatTile from '#lib/ui/StatTile.svelte';
	import TextField from '#lib/ui/TextField.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	type View = 'all' | 'new' | 'missing';
	let filter = $state('');
	let view = $state<View>('all');

	const fields = $derived(data.catalog.fields);
	// Which tool each value comes from: the connector's name, and what it can't provide at all.
	const byId = $derived(new Map(data.sources.map((s) => [s.id, s])));
	const nameOf = (id: string) => byId.get(id)?.name ?? (id === 'gitprompt' ? 'GitPrompt' : id);
	const lacks = (id: string) => {
		const s = byId.get(id);
		return s?.capabilities.length
			? CAPABILITY_IDS.filter((c) => !s.capabilities.includes(c)).map((c) => CAPABILITIES[c].what)
			: [];
	};
	const seen = $derived(fields.filter((f) => f.count > 0));
	const documented = $derived(fields.filter((f) => f.documented));
	const fresh = $derived(seen.filter((f) => !f.documented));
	const missing = $derived(documented.filter((f) => f.count === 0));
	const coverage = $derived(
		documented.length ? (documented.length - missing.length) / documented.length : null
	);

	const matches = (f: Field) =>
		(view === 'all' ||
			(view === 'new' ? f.count > 0 && !f.documented : f.documented && f.count === 0)) &&
		`${f.source} ${f.event} ${f.path}`.toLowerCase().includes(filter.toLowerCase());

	/** Shown fields grouped by source, then event, busiest events first. */
	const groups = $derived.by(() => {
		const bySource = new Map<string, Map<string, Field[]>>();
		for (const f of fields.filter(matches)) {
			const events = bySource.get(f.source) ?? new Map<string, Field[]>();
			events.set(f.event, [...(events.get(f.event) ?? []), f]);
			bySource.set(f.source, events);
		}
		return [...bySource].map(([source, events]) => ({
			source,
			events: [...events]
				.map(([event, list]) => ({
					event,
					fields: list.sort((a, b) => b.count - a.count || a.path.localeCompare(b.path)),
					seen: list.filter((f) => f.count > 0).length,
					count: Math.max(...list.map((f) => f.count))
				}))
				.sort((a, b) => b.count - a.count || a.event.localeCompare(b.event))
		}));
	});
	const shown = $derived(
		groups.reduce((a, g) => a + g.events.reduce((b, e) => b + e.fields.length, 0), 0)
	);
</script>

<svelte:head><title>Fields · GitPrompt</title></svelte:head>

<PageHeader
	title="Fields"
	description="Every field seen in captured payloads, next to what each tool documents. Use it to decide which metadata to build on. Nothing here leaves this machine."
/>

<div class="tiles">
	<StatTile
		label="Fields seen"
		icon={Eye}
		value={integer(seen.length)}
		hint="across {plural(new Set(seen.map((f) => `${f.source}|${f.event}`)).size, 'event type')}"
	/>
	<StatTile
		label="Documented and seen"
		icon={BookCheck}
		value={coverage === null ? '—' : percent(coverage)}
		hint="{integer(documented.length - missing.length)} of {integer(
			documented.length
		)} documented fields"
	/>
	<StatTile
		label="Seen, not documented"
		icon={Sparkles}
		value={integer(fresh.length)}
		hint="Metadata the docs don't list yet"
	/>
	<StatTile
		label="Documented, never seen"
		icon={EyeOff}
		value={integer(missing.length)}
		hint="Not sent in what was captured"
	/>
</div>

<div class="controls">
	<div class="search">
		<TextField
			label="Filter fields"
			hideLabel
			type="search"
			icon={Search}
			bind:value={filter}
			placeholder="Filter by source, event or field"
			clearable
		/>
	</div>
	<SegmentedControl
		label="Which fields"
		bind:value={view}
		options={[
			{ value: 'all', label: 'All' },
			{ value: 'new', label: 'Not documented' },
			{ value: 'missing', label: 'Never seen' }
		]}
	/>
	<span class="muted count" aria-live="polite">{plural(shown, 'field')}</span>
</div>

{#if !groups.length}
	<EmptyState title="No fields match">Try another word, or show all fields.</EmptyState>
{/if}

{#each groups as group (group.source)}
	<section class="source" aria-labelledby="fields-{group.source}">
		<h2 id="fields-{group.source}">
			{nameOf(group.source)}
			{#if nameOf(group.source) !== group.source}<span class="mono id">{group.source}</span>{/if}
		</h2>
		{#if lacks(group.source).length}
			<p class="muted lacks">
				Not available from this connector: {lacks(group.source).join(', ')}.
			</p>
		{/if}
		<div class="events">
			{#each group.events as e (e.event)}
				<Disclosure open={filter.length > 0 || view !== 'all'}>
					{#snippet summary()}<span class="mono event">{e.event}</span>{/snippet}
					{#snippet meta()}
						<span class="coverage" aria-label="{e.seen} of {e.fields.length} fields seen">
							<span class="bar"
								><span style:width="{(e.seen / e.fields.length) * 100}%"></span></span
							>
							<span class="num">{e.seen}/{e.fields.length}</span>
						</span>
						{#if e.count}<span class="num">{integer(e.count)}×</span>{/if}
					{/snippet}
					<div class="table-wrap">
						<table>
							<caption class="sr-only">Fields of {e.event}</caption>
							<thead>
								<tr
									><th scope="col">Field</th><th scope="col" class="n">Seen</th><th scope="col"
										>Docs</th
									><th scope="col">Latest value</th></tr
								>
							</thead>
							<tbody>
								{#each e.fields as f (f.path)}
									<tr class={[f.count === 0 && 'unseen']}>
										<td><code>{f.path}</code></td>
										<td class="n num">{integer(f.count)}</td>
										<td>
											{#if f.documented}
												<Badge size="sm" tone={f.count ? 'success' : 'neutral'}
													>{f.count ? 'Documented' : 'Documented, unseen'}</Badge
												>
											{:else}
												<Badge size="sm" tone="agent">Not documented</Badge>
											{/if}
										</td>
										<td class="sample" title={f.sample == null ? '' : inline(f.sample)}
											>{f.sample == null ? '' : inline(f.sample)}</td
										>
									</tr>
								{/each}
							</tbody>
						</table>
					</div>
				</Disclosure>
			{/each}
		</div>
	</section>
{/each}

<div class="grid">
	<Panel
		title="Derived by GitPrompt"
		description="Computed on read from what was stored, so improvements apply to old prompts too."
	>
		<dl class="glossary">
			{#each Object.entries(data.catalog.derived) as [name, meaning] (name)}
				<dt><code>{name}</code></dt>
				<dd>{meaning}</dd>
			{/each}
		</dl>
	</Panel>
	<Panel title="Not captured, and why">
		<dl class="glossary">
			{#each data.catalog.not_captured as n (`${n.source}|${n.event}`)}
				<dt><code>{n.event}</code> <span class="muted">{nameOf(n.source)}</span></dt>
				<dd>{n.reason}</dd>
			{/each}
		</dl>
	</Panel>
</div>

<style>
	.tiles {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(100%, 12rem), 1fr));
		gap: var(--space-3);
		margin-bottom: var(--space-6);
	}

	.controls {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-3);
		margin-bottom: var(--space-5);
	}

	.search {
		flex: 1 1 18rem;
		max-width: 30rem;
	}

	.count {
		font-size: var(--text-sm);
	}

	.source {
		display: grid;
		gap: var(--space-2);
		margin-bottom: var(--space-6);
	}

	.source h2 {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: var(--space-2);
		font-size: var(--text-lg);
	}

	.source h2 .id {
		color: var(--color-text-muted);
		font-size: var(--text-xs);
		font-weight: var(--weight-regular, 400);
	}

	.lacks {
		margin-top: calc(-1 * var(--space-1));
		font-size: var(--text-sm);
	}

	.events {
		padding: var(--space-2);
		border-radius: var(--radius-lg);
		background: var(--color-surface);
		box-shadow: inset 0 0 0 1px var(--color-border);
	}

	.event {
		font-size: var(--text-sm);
	}

	.coverage {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
	}

	.coverage .num {
		min-width: 3.25rem;
		text-align: right;
	}

	.bar {
		width: 4rem;
		height: 4px;
		border-radius: var(--radius-full);
		background: color-mix(in srgb, var(--color-accent) 16%, transparent);
	}

	.bar span {
		display: block;
		height: 100%;
		border-radius: var(--radius-full);
		background: var(--color-accent);
	}

	.table-wrap {
		overflow-x: auto;
	}

	table {
		width: 100%;
		font-size: var(--text-sm);
	}

	th {
		padding: var(--space-1_5) var(--space-3) var(--space-1_5) 0;
		border-bottom: var(--border-hairline) solid var(--color-border);
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
		font-weight: var(--weight-medium);
		text-align: left;
	}

	td {
		padding: var(--space-1_5) var(--space-3) var(--space-1_5) 0;
		border-bottom: var(--border-hairline) solid var(--color-border-subtle);
		vertical-align: top;
	}

	td code {
		font-size: var(--text-xs);
	}

	.n {
		text-align: right;
	}

	.unseen td {
		color: var(--color-text-muted);
	}

	.sample {
		max-width: 28rem;
		overflow: hidden;
		color: var(--color-text-secondary);
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.grid {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(100%, 26rem), 1fr));
		gap: var(--space-5);
		align-items: start;
	}

	.glossary {
		display: grid;
		gap: var(--space-1) 0;
		font-size: var(--text-sm);
	}

	.glossary dt {
		margin-top: var(--space-2);
	}

	.glossary dd {
		margin: 0;
		color: var(--color-text-secondary);
	}
</style>
