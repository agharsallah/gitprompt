<script lang="ts">
	import { CircleCheck, Coins, GitCommitHorizontal, Hourglass, Keyboard } from '@lucide/svelte';
	import type { Doorway } from '#lib/insights/filters.ts';
	import type { Overview } from '#lib/insights/overview.ts';
	import { ratio, type Slot } from '#lib/insights/series.ts';
	import { duration, integer, money, percent, plural } from '#lib/shared/format.ts';
	import { note, type Availability } from '#lib/sources/capabilities.ts';
	import StatTile from '#lib/ui/StatTile.svelte';

	/** The headline numbers of the filtered prompts, each a link to the prompts it counts. */
	let {
		data,
		slots,
		link,
		cost
	}: { data: Overview; slots: Slot[]; link: Doorway; cost: Availability } = $props();

	// Without cost data: say why (a layer to connect, or a tool that can't report it).
	const noCost = $derived(
		cost.connectable.length
			? `Needs ${cost.connectable[0].tool}’s ${cost.connectable[0].layer.toLowerCase()}`
			: (note(cost) ?? 'No cost reported')
	);

	const t = $derived(data.activity.totals);
	const outcome = (key: string) => data.outcomes.groups.find((g) => g.key === key)?.prompts ?? 0;
	const done = $derived(outcome('done'));
	const failed = $derived(outcome('failed'));
	const usage = $derived(t.api_requests > 0);
</script>

<div class="tiles">
	<StatTile
		label="Prompts"
		icon={Keyboard}
		value={integer(t.prompts)}
		hint="in {plural(t.sessions, 'session')}"
		trend={slots.map((s) => s.bucket?.prompts ?? 0)}
		href={link()}
	/>
	<StatTile
		label="Spend"
		icon={Coins}
		value={usage ? money(t.cost_usd) : '—'}
		hint={usage ? `${money(t.cost_usd / t.prompts)} per prompt` : noCost}
		trend={usage ? slots.map((s) => s.bucket?.cost_usd ?? 0) : undefined}
	/>
	<StatTile
		label="Typical turn"
		icon={Hourglass}
		value={duration(t.response_s_p50)}
		hint={t.response_s_p90 !== null
			? `1 in 10 take over ${duration(t.response_s_p90)}`
			: 'No finished turn yet'}
	/>
	<StatTile
		label="Finished"
		icon={CircleCheck}
		value={done + failed ? percent(ratio(done, done + failed)) : '—'}
		hint={failed ? `${plural(failed, 'turn')} failed` : 'No failed turns'}
		href={failed ? link({ outcome: ['failed'] }) : undefined}
	/>
	<StatTile
		label="Agent commits"
		icon={GitCommitHorizontal}
		value={integer(t.git_commits)}
		hint="after {plural(t.with_commit, 'prompt')}"
		href={t.with_commit ? link({ has: ['commit'] }) : undefined}
	/>
</div>

<style>
	.tiles {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(100%, 10.5rem), 1fr));
		gap: var(--space-3);
	}
</style>
