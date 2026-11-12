<script lang="ts">
	import { Plug, Power, PowerOff } from '@lucide/svelte';
	import { refreshAll } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { api } from '#lib/api/client.ts';
	import type { Source, SpecProblem } from '#lib/api/types.ts';
	import { relativePath } from '#lib/shared/format.ts';
	import { Task } from '#lib/shared/task.svelte.ts';
	import SpecProblems from '#lib/sources/SpecProblems.svelte';
	import StateRow from '#lib/sources/StateRow.svelte';
	import { layerName, tools } from '#lib/sources/state.ts';
	import Badge from '#lib/ui/Badge.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import Panel from '#lib/ui/Panel.svelte';

	/**
	 * Each tool, the state of its connectors and the spec file each runs on; turn a connector on or
	 * off here. Connecting (which edits the tool's settings) happens on the Connectors page.
	 */
	let { sources, problems = [] }: { sources: Source[]; problems?: SpecProblem[] } = $props();

	const list = $derived(tools(sources));
	const task = new Task();
	let busy = $state<string | null>(null);

	const toggle = (s: Source) => {
		busy = s.id;
		void task
			.run(async () => {
				await api.setEnabled(s.id, !s.enabled);
				await refreshAll();
			})
			.finally(() => (busy = null));
	};
</script>

<Panel
	title="Connectors"
	description="The tools GitPrompt captures from. Every change to a tool's settings is reviewed as a diff first. A connector you turn off keeps its entries in the tool's settings, but what it sends is discarded."
>
	{#snippet actions()}
		<Button href={resolve('/sources')} icon={Plug} size="sm">Manage connectors</Button>
	{/snippet}
	<div class="tools">
		<SpecProblems {problems} />
		{#each list as tool (tool.id)}
			<section aria-labelledby="summary-{tool.id}">
				<h3 id="summary-{tool.id}">
					<a href="{resolve('/sources')}#tool-{tool.id}">{tool.name}</a>
				</h3>
				<ul>
					{#each tool.layers as layer (layer.id)}
						<li>
							<StateRow source={layer} />
							<div class="manage">
								<Badge size="sm" variant="outline" tone={layer.spec_problem ? 'warning' : 'neutral'}
									>{layer.spec.origin === 'user' ? 'Your spec' : 'Bundled spec'}</Badge
								>
								<span class="mono path" title={layer.spec.path}
									>{layer.spec.origin === 'user'
										? relativePath(layer.spec.path)
										: layer.spec.name}</span
								>
								<Button
									size="sm"
									variant="ghost"
									icon={layer.enabled ? PowerOff : Power}
									loading={busy === layer.id}
									disabled={!!busy}
									onclick={() => toggle(layer)}
									>{layer.enabled ? 'Turn off' : 'Turn on'}
									<span class="sr-only">{layerName(layer)} of {tool.name}</span></Button
								>
							</div>
						</li>
					{/each}
				</ul>
			</section>
		{/each}
		{#if task.error}<Callout tone="danger" live>{task.error}</Callout>{/if}
	</div>
</Panel>

<style>
	.tools {
		display: grid;
		gap: var(--space-4);
	}

	section {
		display: grid;
		gap: var(--space-2);
	}

	h3 {
		font-size: var(--text-md);
	}

	ul {
		display: grid;
		gap: var(--space-3);
	}

	.manage {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1) var(--space-2);
		margin-top: var(--space-1);
	}

	.path {
		min-width: 0;
		color: var(--color-text-muted);
		font-size: var(--text-xs);
		overflow-wrap: anywhere;
	}
</style>
