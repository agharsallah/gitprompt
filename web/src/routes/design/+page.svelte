<script lang="ts">
	import { Plug, Search, Sparkles, Star, Tag, Trash2 } from '@lucide/svelte';
	import OutcomeBadge from '#lib/prompts/OutcomeBadge.svelte';
	import Badge from '#lib/ui/Badge.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import Chip from '#lib/ui/Chip.svelte';
	import CodeBlock from '#lib/ui/CodeBlock.svelte';
	import Disclosure from '#lib/ui/Disclosure.svelte';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import KeyValue from '#lib/ui/KeyValue.svelte';
	import PageHeader from '#lib/ui/PageHeader.svelte';
	import Panel from '#lib/ui/Panel.svelte';
	import RailNode from '#lib/ui/RailNode.svelte';
	import SegmentedControl from '#lib/ui/SegmentedControl.svelte';
	import StatTile from '#lib/ui/StatTile.svelte';
	import Tabs from '#lib/ui/Tabs.svelte';
	import TextField from '#lib/ui/TextField.svelte';
	import ToggleChip from '#lib/ui/ToggleChip.svelte';
	import BarList from '#lib/ui/charts/BarList.svelte';
	import ColumnChart from '#lib/ui/charts/ColumnChart.svelte';
	import Heatmap from '#lib/ui/charts/Heatmap.svelte';
	import Legend from '#lib/ui/charts/Legend.svelte';
	import Meter from '#lib/ui/charts/Meter.svelte';
	import SegmentedBar from '#lib/ui/charts/SegmentedBar.svelte';
	import Sparkline from '#lib/ui/charts/Sparkline.svelte';
	import { KIND_INFO, TOOL_KINDS } from '#lib/insights/tools.ts';

	const RAMPS = {
		gray: ['0', '25', '50', '75', '100', '150', '200', '300', '400', '500', '600', '700', '750', '800', '850', '900', '925', '950'],
		lagoon: ['50', '100', '200', '300', '400', '500', '600', '700', '800', '900', '950'],
		iris: ['50', '100', '200', '300', '400', '500', '600', '700', '800', '950'],
		amber: ['50', '100', '300', '400', '600', '700', '950'],
		red: ['50', '100', '300', '500', '600', '950'],
		green: ['50', '100', '300', '500', '600', '950'],
		blue: ['50', '100', '300', '500', '600', '950']
	};

	const SEMANTIC = {
		Surfaces: ['canvas', 'surface', 'surface-raised', 'surface-sunken', 'surface-hover', 'surface-selected'],
		Text: ['text', 'text-secondary', 'text-muted', 'text-disabled'],
		Lines: ['border-subtle', 'border', 'border-strong', 'border-control'],
		'You (brand)': ['accent', 'accent-hover', 'accent-text', 'accent-soft', 'accent-border'],
		'The agent': ['agent', 'agent-text', 'agent-soft', 'agent-border'],
		Status: ['success-solid', 'warning-solid', 'danger-solid', 'info-solid', 'star']
	};

	const TYPE = [
		['5xl', '48', 'One hero figure per view'],
		['4xl', '34', 'Display'],
		['3xl', '28', 'Page titles'],
		['2xl', '23', 'Stat values'],
		['xl', '19', 'Section titles, figures'],
		['lg', '16', 'Reading: prompts and answers'],
		['md', '14', 'Interface base'],
		['sm', '13', 'Secondary text, tables'],
		['xs', '12', 'Labels, captions'],
		['2xs', '11', 'Axis ticks']
	];
	const SPACE = ['0_5', '1', '1_5', '2', '2_5', '3', '4', '5', '6', '8', '10', '12', '16'];
	const RADII = ['xs', 'sm', 'md', 'lg', 'xl', 'full'];

	let segment = $state<'day' | 'week' | 'month'>('week');
	let tab = $state<'one' | 'two' | 'three'>('one');
	let text = $state('');
	let toggled = $state(true);
	let pressed = $state(false);

	const heat = Array.from({ length: 7 }, (_, r) =>
		Array.from({ length: 24 }, (_, c) => Math.max(0, Math.round(Math.sin((c - 6) / 4) * 6 + (r < 5 ? 4 : -2) + ((r * c) % 3))))
	);
</script>

<svelte:head><title>Design system · GitPrompt</title></svelte:head>

<PageHeader
	title="Design system"
	description="The tokens and components the dashboard is built from, rendered live in the current theme. Switch the theme in the sidebar to check both."
/>

<div class="design">
	<Panel title="Principles">
		<ol class="principles">
			<li><strong>The rail is the signature.</strong> Sessions are lines, prompts are nodes whose shape and fill say how the turn ended. Everything else stays quiet.</li>
			<li><strong>Color means something.</strong> Lagoon is you, iris is the agent, amber asks for attention, red is failure. Never decoration, never color alone.</li>
			<li><strong>Flat, with tone for depth.</strong> Hairlines and surface steps separate things; only floating layers cast a shadow.</li>
			<li><strong>Every chart can be read without hover.</strong> Values are printed or listed, tooltips only add, and each chart has a table twin.</li>
			<li><strong>Words do one job each.</strong> Sentence case, plain verbs, errors that say what to do next.</li>
		</ol>
	</Panel>

	<Panel title="Semantic color" description="What components use. Each resolves per theme with light-dark().">
		<div class="semantic">
			{#each Object.entries(SEMANTIC) as [group, tokens] (group)}
				<section>
					<h3>{group}</h3>
					<ul class="tokens">
						{#each tokens as token (token)}
							<li>
								<span class="chip-swatch" style:background="var(--color-{token})"></span>
								<code>--color-{token}</code>
							</li>
						{/each}
					</ul>
				</section>
			{/each}
		</div>
	</Panel>

	<Panel title="Primitive ramps" description="Raw steps the semantic tokens point to. Components never use these directly.">
		<div class="ramps">
			{#each Object.entries(RAMPS) as [name, steps] (name)}
				<div class="ramp">
					<span class="ramp-name">{name}</span>
					<div class="steps">
						{#each steps as step (step)}
							<span class="step" style:background="var(--{name}-{step})" title="--{name}-{step}"><span class="sr-only">--{name}-{step}</span></span>
						{/each}
					</div>
				</div>
			{/each}
		</div>
	</Panel>

	<Panel title="Data color" description="Categorical slots in validated order (colorblind-safe for adjacent pairs, both themes), and the sequential ramp for magnitude.">
		<div class="stack">
			<Legend
				label="Tool kinds"
				items={TOOL_KINDS.map((k) => ({ label: `${KIND_INFO[k].label}`, color: KIND_INFO[k].color }))}
			/>
			<div class="steps seq">
				{#each [0, 1, 2, 3, 4, 5, 6] as s (s)}<span class="step" style:background="var(--chart-seq-{s})" title="--chart-seq-{s}"></span>{/each}
			</div>
		</div>
	</Panel>

	<Panel title="Type" description="Instrument Sans Variable for everything, headings slightly condensed; Commit Mono only for code, paths and field names.">
		<ul class="type">
			{#each TYPE as [size, px, use] (size)}
				<li>
					<code>--text-{size}</code>
					<span class="sample" style:font-size="var(--text-{size})">The prompt that worked</span>
					<span class="muted">{px}px, {use}</span>
				</li>
			{/each}
		</ul>
		<p class="mono-sample"><code>git log --oneline -5</code> Commit Mono</p>
	</Panel>

	<div class="grid two">
		<Panel title="Space" description="A 4px grid.">
			<ul class="space">
				{#each SPACE as s (s)}
					<li><code>--space-{s}</code><span class="bar" style:width="var(--space-{s})"></span></li>
				{/each}
			</ul>
		</Panel>
		<Panel title="Radius and elevation" description="Radius grows with size. Only overlays have a shadow.">
			<div class="radii">
				{#each RADII as r (r)}<span class="radius" style:border-radius="var(--radius-{r})"><code>{r}</code></span>{/each}
			</div>
			<div class="overlay-sample">--shadow-overlay</div>
		</Panel>
	</div>

	<Panel title="Buttons and toggles">
		<div class="row">
			<Button variant="primary" icon={Plug}>Connect</Button>
			<Button>Secondary</Button>
			<Button variant="ghost">Ghost</Button>
			<Button variant="danger" icon={Trash2}>Disconnect</Button>
			<Button loading>Saving</Button>
			<Button disabled>Disabled</Button>
			<Button icon={Star} label="Star" pressed={pressed} onclick={() => (pressed = !pressed)} />
			<Button size="sm" variant="primary">Small</Button>
			<Button size="sm" icon={Search} label="Search" />
		</div>
		<div class="row">
			<SegmentedControl label="Period" bind:value={segment} options={[{ value: 'day', label: 'Day' }, { value: 'week', label: 'Week' }, { value: 'month', label: 'Month' }]} />
			<ToggleChip bind:checked={toggled} icon={Star} count={12}>Starred</ToggleChip>
			<Chip icon={Tag}>#refactor</Chip>
			<Chip onremove={() => {}} removeLabel="Remove filter">repo: gitprompt</Chip>
		</div>
		<div class="row fields">
			<TextField label="Search prompts" icon={Search} bind:value={text} placeholder="Search prompt text" clearable />
			<TextField label="Tag" size="sm" placeholder="Small field" hint="Hints sit under the field." />
		</div>
	</Panel>

	<Panel title="Status">
		<div class="row">
			<OutcomeBadge outcome="done" />
			<OutcomeBadge outcome="failed" />
			<OutcomeBadge outcome="running" />
			<OutcomeBadge outcome="open" />
		</div>
		<div class="row">
			{#each ['neutral', 'accent', 'agent', 'success', 'warning', 'danger', 'info'] as tone (tone)}
				<Badge tone={tone as 'neutral'}>{tone}</Badge>
			{/each}
			<Badge variant="outline">outline</Badge>
			<Badge dot tone="success">with dot</Badge>
			<Badge icon={Sparkles} tone="agent">Opus 5.5</Badge>
		</div>
		<div class="row rail-sample">
			{#each ['done', 'failed', 'running', 'open'] as o (o)}
				<span class="rail-node"><RailNode outcome={o as 'done'} size={18} /><code>{o}</code></span>
			{/each}
		</div>
		<div class="stack">
			<Callout tone="info" title="Information">Something worth knowing about the content around it.</Callout>
			<Callout tone="success">Connected. The previous file was backed up.</Callout>
			<Callout tone="warning" title="Possible secret in this prompt">Hide it so it is not kept.</Callout>
			<Callout tone="danger" title="The turn ended on an error">rate_limit_error</Callout>
		</div>
	</Panel>

	<Panel title="Figures and charts">
		<div class="tiles">
			<StatTile label="Spend" value="$12.40" hint="$0.08 per prompt" trend={[2, 4, 3, 6, 5, 8, 7]} />
			<StatTile label="Tokens" value="4.2M" hint="81% from cache"><Meter value={0.81} label="From cache" /></StatTile>
			<StatTile label="Typical turn" value="42s" hint="1 in 10 take over 3m 05s" />
		</div>
		<div class="grid two charts">
			<div class="stack">
				<SegmentedBar
					variant="tall"
					label="Tokens by kind"
					parts={[
						{ key: 'i', label: 'Input', value: 12, color: 'var(--chart-1)' },
						{ key: 'o', label: 'Output', value: 8, color: 'var(--chart-2)' },
						{ key: 'w', label: 'Cache write', value: 20, color: 'var(--chart-3)' },
						{ key: 'r', label: 'Cache read', value: 60, color: 'var(--chart-4)' }
					]}
				/>
				<BarList label="Tools" items={[{ key: 'b', label: 'Bash', value: 42, color: 'var(--chart-3)' }, { key: 'r', label: 'Read', value: 30, color: 'var(--chart-1)' }, { key: 'e', label: 'Edit', value: 12, color: 'var(--chart-2)' }]} />
				<Sparkline values={[1, 3, 2, 5, 4, 7, 6, 9]} width={160} height={36} label="Sample trend" />
			</div>
			<div class="stack">
				<ColumnChart label="Prompts per day" emphasis="7" data={[3, 8, 5, 12, 9, 2, 6].map((v, i) => ({ key: String(i + 1), label: `Day ${i + 1}`, tick: ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'][i], value: v }))} />
				<Heatmap label="Sample heatmap" values={heat} rows={['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun']} columns={Array.from({ length: 24 }, (_, h) => String(h))} columnTick={(c, i) => (i % 6 === 0 ? c : '')} describe={(r, c, v) => `${v} on ${r} at ${c}:00`} />
			</div>
		</div>
	</Panel>

	<Panel title="Content">
		<Tabs label="Sample tabs" bind:selected={tab} tabs={[{ id: 'one', label: 'Activity', count: 12 }, { id: 'two', label: 'Usage' }, { id: 'three', label: 'Disabled', disabled: true }]}>
			{#snippet panel(id)}
				{#if id === 'one'}
					<KeyValue data={{ git_branch: 'main', git_ahead: 2, files: ['src/app.ts', 'README.md'], cwd: '/Users/me/project', worktree: false }} rawKeys />
				{:else}
					<p class="muted">Usage panel.</p>
				{/if}
			{/snippet}
		</Tabs>
		<div class="stack">
			<CodeBlock label="settings.json" diff code={'@@ hooks @@\n   "hooks": {\n+    "UserPromptSubmit": [ ... ],\n-    "Stop": [ ... ]\n   }'} />
			<Disclosure summary="A disclosure">Hidden detail, opened on demand.</Disclosure>
			<EmptyState title="No prompts yet">Connect a source, then type a prompt in it.</EmptyState>
		</div>
	</Panel>
</div>

<style>
	.design {
		display: grid;
		gap: var(--space-5);
	}

	.principles {
		display: grid;
		gap: var(--space-2);
		max-width: var(--reading-max);
		padding-left: var(--space-5);
		list-style: decimal;
		color: var(--color-text-secondary);
	}

	.principles strong {
		color: var(--color-text);
	}

	.semantic {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(14rem, 1fr));
		gap: var(--space-5);
	}

	h3 {
		margin-bottom: var(--space-2);
		font-size: var(--text-sm);
	}

	.tokens {
		display: grid;
		gap: var(--space-1_5);
	}

	.tokens li {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}

	.tokens code {
		font-size: var(--text-xs);
	}

	.chip-swatch {
		width: 1.5rem;
		height: 1.5rem;
		border-radius: var(--radius-sm);
		box-shadow: inset 0 0 0 1px var(--color-border);
	}

	.ramps {
		display: grid;
		gap: var(--space-2);
	}

	.ramp {
		display: grid;
		grid-template-columns: 4rem minmax(0, 1fr);
		align-items: center;
		gap: var(--space-3);
	}

	.ramp-name {
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}

	.steps {
		display: flex;
		gap: 2px;
	}

	.step {
		flex: 1;
		height: 2rem;
		border-radius: var(--radius-xs);
		box-shadow: inset 0 0 0 1px light-dark(rgb(0 0 0 / 0.05), rgb(255 255 255 / 0.06));
	}

	.seq .step {
		max-width: 3rem;
	}

	.stack {
		display: grid;
		gap: var(--space-4);
	}

	.type {
		display: grid;
		gap: var(--space-3);
	}

	.type li {
		display: grid;
		grid-template-columns: 7rem minmax(0, 1fr) 14rem;
		align-items: baseline;
		gap: var(--space-4);
	}

	.type code,
	.type .muted {
		font-size: var(--text-xs);
	}

	.sample {
		overflow: hidden;
		line-height: var(--leading-tight);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.mono-sample {
		margin-top: var(--space-4);
		color: var(--color-text-secondary);
	}

	.space {
		display: grid;
		gap: var(--space-1_5);
	}

	.space li {
		display: grid;
		grid-template-columns: 7rem 1fr;
		align-items: center;
	}

	.space code {
		font-size: var(--text-xs);
	}

	.space .bar {
		height: 0.75rem;
		border-radius: 2px;
		background: var(--color-accent);
	}

	.radii {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-3);
		margin-bottom: var(--space-5);
	}

	.radius {
		display: grid;
		place-items: center;
		width: 4rem;
		height: 4rem;
		background: var(--color-accent-soft);
		box-shadow: inset 0 0 0 1px var(--color-accent-border);
		font-size: var(--text-xs);
	}

	.overlay-sample {
		display: inline-block;
		padding: var(--space-3) var(--space-4);
		border-radius: var(--radius-md);
		background: var(--color-surface-raised);
		box-shadow: var(--shadow-overlay);
		font-family: var(--font-mono);
		font-size: var(--text-xs);
	}

	.grid {
		display: grid;
		gap: var(--space-5);
		align-items: start;
	}

	.two {
		grid-template-columns: repeat(auto-fit, minmax(min(100%, 24rem), 1fr));
	}

	.charts {
		margin-top: var(--space-5);
	}

	.row {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2) var(--space-3);
		margin-bottom: var(--space-4);
	}

	.fields > :global(*) {
		flex: 1 1 14rem;
	}

	.rail-sample {
		gap: var(--space-6);
	}

	.rail-node {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		font-size: var(--text-xs);
	}

	.tiles {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(100%, 12rem), 1fr));
		gap: var(--space-3);
	}

	@media (max-width: 48rem) {
		.type li {
			grid-template-columns: minmax(0, 1fr);
			gap: 0;
		}
	}
</style>
