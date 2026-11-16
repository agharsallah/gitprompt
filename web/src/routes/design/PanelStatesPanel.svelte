<script lang="ts">
	import type { Availability } from '#lib/sources/capabilities.ts';
	import { missingNotice } from '#lib/sources/missing.ts';
	import BarList from '#lib/ui/charts/BarList.svelte';
	import Panel from '#lib/ui/Panel.svelte';
	import { PANEL_STATE_KINDS, type PanelState, type PanelStateKind } from '#lib/ui/panelState.ts';

	/** The states every panel, list and story shows, drawn by lib/ui/PanelState.svelte. */

	const NO_COST: Availability = {
		capability: 'cost',
		available: [],
		connectable: [],
		updatable: [],
		unavailable: ['OpenAI Codex CLI']
	};

	const SAMPLES: Record<PanelStateKind, { say: string; state: PanelState }> = {
		loading: {
			say: 'The data is on its way: a skeleton shaped like what comes.',
			state: { kind: 'loading', label: 'Loading the sample', body: 'rows', rows: 3 }
		},
		empty: {
			say: 'Loaded, nothing to show: what that means, and the next step.',
			state: {
				kind: 'empty',
				title: 'No starred prompts here',
				message: 'Star a prompt that worked well to find it again here.',
				next: { label: 'Open the prompts', href: '#panel-states' }
			}
		},
		unsupported: {
			say: 'No connected tool reports it: says which one could.',
			state: { kind: 'unsupported', ...missingNotice('costs', NO_COST) }
		},
		error: {
			say: 'Loading failed: said as everywhere, with a way to try again.',
			state: {
				kind: 'error',
				error: new TypeError('Failed to fetch'),
				what: 'this panel',
				retry: () => {}
			}
		},
		fresh: {
			say: 'The content, with a live chip: new data arrived.',
			state: { kind: 'fresh', count: 3, unit: 'prompt' }
		},
		sparse: {
			say: 'Too little data to draw anything honest: the least it needs.',
			state: { kind: 'sparse', minimum: 10, unit: 'prompt', have: 4 }
		},
		ready: { say: 'The content itself, with nothing added around it.', state: { kind: 'ready' } }
	};
</script>

<Panel
	title="Panel states"
	description="Every panel, list and story shows one of these through lib/ui/PanelState.svelte (a Panel takes it as its state). The union in lib/ui/panelState.ts is closed: a page picks a state, it cannot invent one."
>
	<div id="panel-states" class="grid two">
		{#each PANEL_STATE_KINDS as kind (kind)}
			<Panel
				title={kind}
				description={SAMPLES[kind].say}
				tone="sunken"
				level={3}
				state={SAMPLES[kind].state}
			>
				<BarList
					label="Sample content"
					items={[
						{ key: 'a', label: 'Fix the failing test', value: 5 },
						{ key: 'b', label: 'Explain this stack trace', value: 3 }
					]}
				/>
			</Panel>
		{/each}
	</div>
</Panel>
