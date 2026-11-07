<script lang="ts">
	import { CircleCheck, Coins, FilePen, Hourglass, Layers } from '@lucide/svelte';
	import { refreshAll } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { liveUpdates } from '#lib/api/live.svelte.ts';
	import {
		attention,
		byDay,
		byHour,
		cacheShare,
		craft,
		kindMix,
		lanes,
		lengthVsWork,
		modelMix,
		projects,
		tokenTotal,
		toolUse,
		totals,
		weekHours
	} from '#lib/insights/aggregate.ts';
	import AttentionList from '#lib/insights/AttentionList.svelte';
	import RangePicker from '#lib/insights/RangePicker.svelte';
	import { rangePhrase } from '#lib/insights/range.ts';
	import { KIND_INFO, toolKind, toolLabel } from '#lib/insights/tools.ts';
	import { clock } from '#lib/shared/clock.svelte.ts';
	import {
		compact,
		dayKey,
		dayLabel,
		duration,
		excerpt,
		integer,
		modelName,
		money,
		percent,
		plural,
		shortDuration
	} from '#lib/shared/format.ts';
	import SetupSteps from '#lib/sources/SetupSteps.svelte';
	import BarList from '#lib/ui/charts/BarList.svelte';
	import ColumnChart from '#lib/ui/charts/ColumnChart.svelte';
	import { durationTicks } from '#lib/ui/charts/scale.ts';
	import Heatmap from '#lib/ui/charts/Heatmap.svelte';
	import Lanes from '#lib/ui/charts/Lanes.svelte';
	import Meter from '#lib/ui/charts/Meter.svelte';
	import SegmentedBar from '#lib/ui/charts/SegmentedBar.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import PageHeader from '#lib/ui/PageHeader.svelte';
	import Panel from '#lib/ui/Panel.svelte';
	import StatTile from '#lib/ui/StatTile.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	// The window is several pages of prompts: refetch once capture goes quiet, not on every event.
	$effect(() => liveUpdates(refreshAll, 3000));

	const prompts = $derived(data.window.prompts);
	const now = $derived(data.window.loadedAt);
	const start = $derived(
		data.window.start ??
			new Date(prompts.length ? Date.parse(prompts.at(-1)!.received_at) : now - 86_400_000)
	);
	const hourly = $derived(data.range.id === '24h');
	const t = $derived(totals(prompts, clock.now));
	const buckets = $derived(hourly ? byHour(prompts, start, new Date(now)) : byDay(prompts, start, new Date(now)));
	const projectList = $derived(projects(prompts, byDay(prompts, start, new Date(now))));
	const tools = $derived(toolUse(prompts));
	const kinds = $derived(kindMix(prompts));
	const models = $derived(modelMix(prompts));
	const words = $derived(craft(prompts));
	const lengths = $derived(lengthVsWork(prompts));
	const look = $derived(attention(prompts));
	const starred = $derived(prompts.filter((p) => p.annotation.starred).slice(0, 5));
	const ended = $derived(t.outcomes.done + t.outcomes.failed);
	const cache = $derived(cacheShare(t.tokens));
	const hooked = $derived(data.sources.some((s) => s.connected));
	const telemetry = $derived(data.sources.find((s) => s.id === 'claude-code-otel')?.connected);
	const ever = $derived(data.sources.some((s) => s.total > 0));

	const repoHref = (root?: string) =>
		root ? `${resolve('/prompts')}?repo=${encodeURIComponent(root)}` : undefined;

	const WEEKDAYS = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
	const WEEKDAY_NAMES = ['Mondays', 'Tuesdays', 'Wednesdays', 'Thursdays', 'Fridays', 'Saturdays', 'Sundays'];
	const HOURS = Array.from({ length: 24 }, (_, h) => String(h).padStart(2, '0'));

	const columns = $derived(
		buckets.map((b) => ({
			key: b.key,
			label: hourly
				? `${b.date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}, ${dayLabel(dayKey(b.date), new Date(now)).toLowerCase()}`
				: dayLabel(b.key, new Date(now)),
			tick: hourly
				? b.date.toLocaleTimeString([], { hour: '2-digit' })
				: b.date.toLocaleDateString('en-GB', buckets.length > 10 ? { day: 'numeric', month: 'short' } : { weekday: 'short' }),
			prompts: b.prompts,
			cost: b.cost
		}))
	);
	const summary = $derived(
		`${plural(t.prompts, 'prompt')} in ${plural(t.sessions, 'session')} across ${plural(t.projects, 'project')} ${rangePhrase(data.range)}.`
	);
</script>

<svelte:head><title>Overview · GitPrompt</title></svelte:head>

<PageHeader title="Overview" description={prompts.length ? summary : 'Your prompting, at a glance.'}>
	{#snippet actions()}<RangePicker range={data.range} />{/snippet}
</PageHeader>

<div class="overview">
	{#if !hooked || !ever}
		<SetupSteps sources={data.sources} />
	{/if}

	{#if data.window.capped}
		<Callout tone="info">
			This window holds more prompts than the overview reads at once; the figures cover the newest
			{integer(prompts.length)}.
		</Callout>
	{/if}

	{#if !prompts.length}
		{#if ever}
			<EmptyState title="Nothing {rangePhrase(data.range)}">
				No prompts were captured in this window. Pick a longer range, or open the
				<a href={resolve('/prompts')}>full list of prompts</a>.
			</EmptyState>
		{/if}
	{:else}
		<Panel title="Where your prompts went" description="Each tick is a prompt, on its project's lane, colored and shaped by how its turn ended.">
			<Lanes
				label="Prompts by project over time"
				lanes={lanes(prompts, 7, clock.now).map((l) => ({ ...l, href: repoHref(l.root) }))}
				start={start.getTime()}
				end={now}
				hrefFor={(id) => `${resolve('/prompts')}?id=${id}`}
			/>
		</Panel>

		<div class="tiles">
			<StatTile
				label="Spend"
				icon={Coins}
				value={t.withTelemetry ? money(t.cost) : '—'}
				hint={t.withTelemetry
					? `${money(t.cost / t.withTelemetry)} per prompt`
					: telemetry
						? 'No usage reported yet'
						: 'Connect telemetry to see cost'}
				trend={t.withTelemetry ? columns.map((c) => c.cost) : undefined}
			/>
			<StatTile
				label="Tokens"
				icon={Layers}
				value={t.withTelemetry ? compact(tokenTotal(t.tokens)) : '—'}
				hint={cache !== null ? `${percent(cache)} of prompt tokens from cache` : undefined}
			>
				{#if cache !== null}<Meter value={cache} label="Prompt tokens served from cache" />{/if}
			</StatTile>
			<StatTile
				label="Typical turn"
				icon={Hourglass}
				value={duration(t.responseMedian)}
				hint={t.responseP90 !== null ? `1 in 10 take over ${duration(t.responseP90)}` : undefined}
			/>
			<StatTile
				label="Finished"
				icon={CircleCheck}
				value={ended ? percent(t.outcomes.done / ended) : '—'}
				hint={t.outcomes.failed ? `${plural(t.outcomes.failed, 'turn')} failed` : 'No failed turns'}
				href={t.outcomes.failed ? `${resolve('/prompts')}?show=failed&limit=200` : undefined}
			/>
			<StatTile
				label="Files changed"
				icon={FilePen}
				value={integer(t.filesWritten)}
				hint="{plural(t.commands, 'command')} run"
				href={`${resolve('/prompts')}?show=files&limit=200`}
			/>
		</div>

		<div class="grid two">
			<Panel title="When you prompt" description="Prompts by weekday and hour, in your time zone.">
				<Heatmap
					label="Prompts by weekday and hour"
					values={weekHours(prompts)}
					rows={WEEKDAYS}
					columns={HOURS}
					columnTick={(c, i) => (i % 6 === 0 ? `${c}:00` : '')}
					describe={(r, c, v) => `${plural(v, 'prompt')} on ${WEEKDAY_NAMES[WEEKDAYS.indexOf(r)]} at ${c}:00`}
				/>
			</Panel>
			<Panel title={hourly ? 'Prompts per hour' : 'Prompts per day'}>
				<ColumnChart
					label={hourly ? 'Prompts per hour' : 'Prompts per day'}
					data={columns.map((c) => ({ key: c.key, label: c.label, tick: c.tick, value: c.prompts }))}
					format={(v) => plural(v, 'prompt')}
					axisFormat={compact}
				/>
				{#if t.withTelemetry}
					<h3 class="sub">{hourly ? 'Spend per hour' : 'Spend per day'}</h3>
					<ColumnChart
						label={hourly ? 'Spend per hour' : 'Spend per day'}
						data={columns.map((c) => ({ key: c.key, label: c.label, tick: c.tick, value: c.cost }))}
						height={80}
						format={money}
						axisFormat={money}
						color="var(--chart-2)"
					/>
				{/if}
			</Panel>
		</div>

		<div class="grid three">
			<Panel title="What the agent did" description="{plural(t.toolCalls, 'tool call')}{t.toolFailures ? `, ${integer(t.toolFailures)} failed` : ''}.">
				{#if kinds.length}
					<div class="stack">
						<SegmentedBar
							variant="tall"
							label="Tool calls by kind"
							parts={kinds.map((k) => ({ key: k.kind, label: KIND_INFO[k.kind].label, value: k.count, color: KIND_INFO[k.kind].color }))}
							format={(v) => plural(v, 'call')}
						/>
						<BarList
							label="Most used tools"
							max={7}
							otherLabel="Other tools"
							items={tools.map((tool) => ({
								key: tool.name,
								label: toolLabel(tool.name),
								value: tool.count,
								color: KIND_INFO[toolKind(tool.name)].color,
								mono: tool.name.startsWith('mcp__')
							}))}
						/>
					</div>
				{:else}
					<EmptyState compact title="No tool calls">Turns in this window were answered directly.</EmptyState>
				{/if}
			</Panel>

			<Panel title="How you write" description="Median prompt: {plural(Math.round(words.medianWords ?? 0), 'word')}.">
				<div class="stack">
					<ul class="craft">
						<li><span class="num">{percent(words.withCode / t.prompts)}</span> include a code block</li>
						<li><span class="num">{percent(words.withQuestion / t.prompts)}</span> ask a question</li>
						<li><span class="num">{percent(words.withLink / t.prompts)}</span> link somewhere</li>
						{#if words.withMention}<li><span class="num">{percent(words.withMention / t.prompts)}</span> @-mention a file</li>{/if}
					</ul>
					<h3 class="sub">Typical turn length, by prompt length in words</h3>
					<ColumnChart
						label="Median turn duration by prompt length"
						data={lengths.map((l) => ({ key: l.label, label: `${l.label} words (${plural(l.prompts, 'prompt')})`, tick: l.label, value: l.medianResponse ?? 0 }))}
						height={96}
						format={(v) => duration(v)}
						axisFormat={shortDuration}
						ticks={durationTicks(Math.max(...lengths.map((l) => l.medianResponse ?? 0)))}
						color="var(--chart-1)"
					/>
					{#if words.slashCommands.length}
						<h3 class="sub">Commands you use</h3>
						<BarList label="Slash commands" max={5} items={words.slashCommands.map((c) => ({ key: c.name, label: c.name, value: c.count, mono: true }))} />
					{/if}
				</div>
			</Panel>

			<Panel title="Models" description="Requests by model.">
				{#if models.length}
					<BarList
						label="Requests by model"
						color="var(--color-agent)"
						items={models.map((m) => ({ key: m.model, label: modelName(m.model), value: m.requests, hint: m.model.includes('haiku') ? 'background' : undefined }))}
						format={(v) => compact(v)}
					/>
				{:else}
					<EmptyState compact title="No model data">
						Models come from Claude Code telemetry. <a href={resolve('/sources')}>Connect it</a> to see them.
					</EmptyState>
				{/if}
			</Panel>
		</div>

		<div class="grid two">
			<Panel title="Needs a look">
				<AttentionList attention={look} />
			</Panel>
			<Panel title="Projects" description="By prompts sent.">
				{#snippet actions()}<a class="all" href={resolve('/projects')}>All projects</a>{/snippet}
				<BarList
					label="Prompts by project"
					max={6}
					otherLabel="Other projects"
					items={projectList.map((p) => ({
						key: p.project.key,
						label: p.project.name,
						value: p.prompts,
						hint: p.cost ? money(p.cost) : undefined,
						href: repoHref(p.project.root)
					}))}
				/>
			</Panel>
		</div>

		<Panel title="Starred" description="Prompts you marked as worth keeping.">
			{#if starred.length}
				<ul class="starred">
					{#each starred as p (p.id)}
						<li><a href="{resolve('/prompts')}?id={p.id}">{excerpt(p.prompt, 160)}</a></li>
					{/each}
				</ul>
			{:else}
				<EmptyState compact title="No starred prompts {rangePhrase(data.range)}">
					Star a prompt that worked well to find it again here.
				</EmptyState>
			{/if}
		</Panel>
	{/if}
</div>

<style>
	.overview {
		display: grid;
		gap: var(--space-5);
	}

	.tiles {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(100%, 11rem), 1fr));
		gap: var(--space-3);
	}

	.grid {
		display: grid;
		gap: var(--space-5);
		align-items: start;
	}

	.two {
		grid-template-columns: repeat(auto-fit, minmax(min(100%, 26rem), 1fr));
	}

	.three {
		grid-template-columns: repeat(auto-fit, minmax(min(100%, 19rem), 1fr));
	}

	.stack {
		display: grid;
		gap: var(--space-4);
	}

	.sub {
		margin: var(--space-5) 0 var(--space-2);
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
		font-weight: var(--weight-medium);
		font-stretch: var(--stretch-normal);
		letter-spacing: var(--tracking-normal);
	}

	.stack .sub {
		margin: var(--space-1) 0 calc(-1 * var(--space-2));
	}

	.craft {
		display: grid;
		gap: var(--space-1);
		font-size: var(--text-sm);
	}

	.craft .num {
		display: inline-block;
		min-width: 3ch;
		margin-right: var(--space-1);
		font-weight: var(--weight-semibold);
	}

	.all {
		font-size: var(--text-sm);
	}

	.starred {
		display: grid;
		gap: var(--space-2);
	}

	.starred a {
		color: var(--color-text);
		text-decoration: none;
	}

	.starred a:hover {
		color: var(--color-accent-text);
		text-decoration: underline;
	}
</style>
