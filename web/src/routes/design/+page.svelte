<script lang="ts">
	import PageHeader from '#lib/ui/PageHeader.svelte';
	import Panel from '#lib/ui/Panel.svelte';
	import ChartsPanel from './ChartsPanel.svelte';
	import ColorSection from './ColorSection.svelte';
	import ConnectorStatesPanel from './ConnectorStatesPanel.svelte';
	import ContentPanel from './ContentPanel.svelte';
	import ControlsPanel from './ControlsPanel.svelte';
	import LoadingPanel from './LoadingPanel.svelte';
	import StatusPanel from './StatusPanel.svelte';
	import TypeSection from './TypeSection.svelte';
</script>

<svelte:head><title>Design system · GitPrompt</title></svelte:head>

<PageHeader
	title="Design system"
	description="The tokens and components the dashboard is built from, rendered live in the current theme. Switch the theme in the sidebar to check both."
/>

<div class="design">
	<Panel title="Principles">
		<ol class="principles">
			<li>
				<strong>The rail is the signature.</strong> Sessions are lines, prompts are nodes whose shape
				and fill say how the turn ended. Everything else stays quiet.
			</li>
			<li>
				<strong>Color means something.</strong> Lagoon is you, iris is the agent, amber asks for attention,
				red is failure. Never decoration, never color alone.
			</li>
			<li>
				<strong>Flat, with tone for depth.</strong> Hairlines and surface steps separate things; only
				floating layers cast a shadow.
			</li>
			<li>
				<strong>Every chart can be read without hover.</strong> Values are printed or listed, tooltips
				only add, and each chart has a table twin.
			</li>
			<li>
				<strong>Words do one job each.</strong> Sentence case, plain verbs, errors that say what to do
				next.
			</li>
		</ol>
	</Panel>

	<ColorSection />
	<TypeSection />
	<ControlsPanel />
	<StatusPanel />
	<ConnectorStatesPanel />
	<ChartsPanel />
	<ContentPanel />
	<LoadingPanel />
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

	/* The layouts every section of the page uses: a vertical stack, a row, and a two-column grid.
	   Without weight (:where), so a section's own class refines them. */
	:global(:where(.design) .stack) {
		display: grid;
		gap: var(--space-4);
	}

	:global(:where(.design) .row) {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2) var(--space-3);
		margin-bottom: var(--space-4);
	}

	:global(:where(.design) .grid) {
		display: grid;
		gap: var(--space-5);
		align-items: start;
	}

	:global(:where(.design) .two) {
		grid-template-columns: repeat(auto-fit, minmax(min(100%, 24rem), 1fr));
	}
</style>
