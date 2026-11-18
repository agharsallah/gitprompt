<script lang="ts">
	import type { Capability } from '#lib/api/types.ts';
	import Lazy from '#lib/async/Lazy.svelte';
	import Boundary from '#lib/errors/Boundary.svelte';
	import type { Doorway } from '#lib/insights/filters.ts';
	import type { Overview } from '#lib/insights/overview.ts';
	import RhythmPanel from '#lib/insights/panels/RhythmPanel.svelte';
	import SessionsPanel from '#lib/insights/panels/SessionsPanel.svelte';
	import type { Slot } from '#lib/insights/series.ts';
	import type { WeekStart } from '#lib/insights/week.ts';
	import type { PanelName } from '#lib/shared/usage-actions.ts';
	import type { Availability } from '#lib/sources/capabilities.ts';
	import PanelSkeleton from '#lib/ui/PanelSkeleton.svelte';

	/**
	 * One Overview panel, by its name in the tab table (lib/insights/tabs.ts). Activity, the tab the
	 * page opens on, is part of the page; every other panel's code loads when its tab opens.
	 */
	let {
		name,
		data,
		link,
		slots,
		can,
		weekStart,
		starredHref
	}: {
		name: PanelName;
		data: Overview;
		link: Doorway;
		slots: Slot[];
		can: (capability: Capability) => Availability;
		weekStart: WeekStart;
		starredHref: string;
	} = $props();
</script>

{#snippet panel()}<PanelSkeleton />{/snippet}
{#snippet rows()}<PanelSkeleton body="rows" rows={3} />{/snippet}

{#if name === 'rhythm'}
	<Boundary what="the rhythm panel"><RhythmPanel {data} {weekStart} {link} /></Boundary>
{:else if name === 'sessions'}
	<Boundary what="the sessions panel"><SessionsPanel {data} /></Boundary>
{:else if name === 'origin'}
	<Lazy
		load={() => import('#lib/insights/panels/OriginPanel.svelte')}
		props={{ data, link }}
		label="Loading who wrote the prompts"
		what="the origins panel"
		pending={panel}
	/>
{:else if name === 'commands'}
	<Lazy
		load={() => import('#lib/insights/panels/CommandsPanel.svelte')}
		props={{ data, link, availability: can('session_metrics') }}
		label="Loading slash commands"
		what="the commands panel"
		pending={panel}
	/>
{:else if name === 'writing'}
	<Lazy
		load={() => import('#lib/insights/panels/WritingPanel.svelte')}
		props={{ data, link }}
		label="Loading prompt sizes"
		what="the writing panel"
		pending={panel}
	/>
{:else if name === 'reuse'}
	<Lazy
		load={() => import('#lib/insights/panels/ReusePanel.svelte')}
		props={{ data, link }}
		label="Loading reused prompts"
		what="the reuse panel"
		pending={panel}
	/>
{:else if name === 'spend'}
	<Lazy
		load={() => import('#lib/insights/panels/SpendPanel.svelte')}
		props={{ data, link, availability: can('cost') }}
		label="Loading spend"
		what="the spend panel"
		pending={panel}
	/>
{:else if name === 'models'}
	<Lazy
		load={() => import('#lib/insights/panels/ModelsPanel.svelte')}
		props={{ data, link, availability: can('model') }}
		label="Loading models"
		what="the models panel"
		pending={panel}
	/>
{:else if name === 'activity'}
	<Lazy
		load={() => import('#lib/insights/panels/ActivityPanel.svelte')}
		props={{ data, slots, link, availability: can('cost') }}
		label="Loading spend per day"
		what="the spend per day panel"
		pending={panel}
	/>
{:else if name === 'agent'}
	<Lazy
		load={() => import('#lib/insights/panels/AgentPanel.svelte')}
		props={{ data, link }}
		label="Loading agent work"
		what="the agent panel"
		pending={panel}
	/>
{:else if name === 'delegation'}
	<Lazy
		load={() => import('#lib/insights/panels/DelegationPanel.svelte')}
		props={{ data, link }}
		label="Loading delegation"
		what="the delegation panel"
		pending={panel}
	/>
{:else if name === 'performance'}
	<Lazy
		load={() => import('#lib/insights/panels/PerformancePanel.svelte')}
		props={{ data, link }}
		label="Loading performance"
		what="the performance panel"
		pending={panel}
	/>
{:else if name === 'outcomes'}
	<Lazy
		load={() => import('#lib/insights/panels/OutcomesPanel.svelte')}
		props={{ data, link, availability: can('session_metrics') }}
		label="Loading outcomes"
		what="the outcomes panel"
		pending={panel}
	/>
{:else if name === 'starred'}
	<Lazy
		load={() => import('#lib/insights/panels/StarredPanel.svelte')}
		props={{ prompts: data.starred, href: starredHref }}
		label="Loading starred prompts"
		what="the starred prompts"
		pending={rows}
	/>
{/if}
