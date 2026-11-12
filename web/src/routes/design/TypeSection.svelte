<script lang="ts">
	import Panel from '#lib/ui/Panel.svelte';

	/** Type sizes, the space scale, radii and the one elevation. */

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
</script>

<Panel
	title="Type"
	description="Instrument Sans Variable for everything, headings slightly condensed; Commit Mono only for code, paths and field names."
>
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
	<Panel
		title="Radius and elevation"
		description="Radius grows with size. Only overlays have a shadow."
	>
		<div class="radii">
			{#each RADII as r (r)}
				<span class="radius" style:border-radius="var(--radius-{r})"><code>{r}</code></span>
			{/each}
		</div>
		<div class="overlay-sample">--shadow-overlay</div>
	</Panel>
</div>

<style>
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

	@media (max-width: 48rem) {
		.type li {
			grid-template-columns: minmax(0, 1fr);
			gap: 0;
		}
	}
</style>
