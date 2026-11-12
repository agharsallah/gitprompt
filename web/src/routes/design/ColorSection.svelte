<script lang="ts">
	import Legend from '#lib/ui/charts/Legend.svelte';
	import Panel from '#lib/ui/Panel.svelte';
	import { KIND_INFO, TOOL_KINDS } from '#lib/insights/tools.ts';

	/** Color, from what components use down to the raw ramps, and the colors that carry data. */

	const SEMANTIC = {
		Surfaces: [
			'canvas',
			'surface',
			'surface-raised',
			'surface-sunken',
			'surface-hover',
			'surface-selected'
		],
		Text: ['text', 'text-secondary', 'text-muted', 'text-disabled'],
		Lines: ['border-subtle', 'border', 'border-strong', 'border-control'],
		'You (brand)': ['accent', 'accent-hover', 'accent-text', 'accent-soft', 'accent-border'],
		'The agent': ['agent', 'agent-text', 'agent-soft', 'agent-border'],
		Status: ['success-solid', 'warning-solid', 'danger-solid', 'info-solid', 'star']
	};

	const RAMPS = {
		gray: [
			'0',
			'25',
			'50',
			'75',
			'100',
			'150',
			'200',
			'300',
			'400',
			'500',
			'600',
			'700',
			'750',
			'800',
			'850',
			'900',
			'925',
			'950'
		],
		lagoon: ['50', '100', '200', '300', '400', '500', '600', '700', '800', '900', '950'],
		iris: ['50', '100', '200', '300', '400', '500', '600', '700', '800', '950'],
		amber: ['50', '100', '300', '400', '600', '700', '950'],
		red: ['50', '100', '300', '500', '600', '950'],
		green: ['50', '100', '300', '500', '600', '950'],
		blue: ['50', '100', '300', '500', '600', '950']
	};

	const SEQUENTIAL = [0, 1, 2, 3, 4, 5, 6];
	const kinds = TOOL_KINDS.map((k) => ({ label: KIND_INFO[k].label, color: KIND_INFO[k].color }));
</script>

<Panel
	title="Semantic color"
	description="What components use. Each resolves per theme with light-dark()."
>
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

<Panel
	title="Primitive ramps"
	description="Raw steps the semantic tokens point to. Components never use these directly."
>
	<div class="ramps">
		{#each Object.entries(RAMPS) as [name, steps] (name)}
			<div class="ramp">
				<span class="ramp-name">{name}</span>
				<div class="steps">
					{#each steps as step (step)}
						<span class="step" style:background="var(--{name}-{step})" title="--{name}-{step}"
							><span class="sr-only">--{name}-{step}</span></span
						>
					{/each}
				</div>
			</div>
		{/each}
	</div>
</Panel>

<Panel
	title="Data color"
	description="Categorical slots in validated order (colorblind-safe for adjacent pairs, both themes), and the sequential ramp for magnitude."
>
	<div class="stack">
		<Legend label="Tool kinds" items={kinds} />
		<div class="steps seq">
			{#each SEQUENTIAL as s (s)}
				<span class="step" style:background="var(--chart-seq-{s})" title="--chart-seq-{s}"></span>
			{/each}
		</div>
	</div>
</Panel>

<style>
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
</style>
