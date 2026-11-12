<script lang="ts">
	import { Sparkles } from '@lucide/svelte';
	import type { Outcome, Tone } from '#lib/insights/outcome.ts';
	import OutcomeBadge from '#lib/prompts/OutcomeBadge.svelte';
	import Badge from '#lib/ui/Badge.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import Panel from '#lib/ui/Panel.svelte';
	import RailNode from '#lib/ui/RailNode.svelte';

	/** How status reads: outcome badges, every tone, the rail's nodes and the callouts. */

	const OUTCOMES: Outcome[] = ['done', 'failed', 'running', 'open'];
	const TONES: Tone[] = ['neutral', 'accent', 'agent', 'success', 'warning', 'danger', 'info'];
</script>

<Panel title="Status">
	<div class="row">
		{#each OUTCOMES as outcome (outcome)}<OutcomeBadge {outcome} />{/each}
	</div>
	<div class="row">
		{#each TONES as tone (tone)}<Badge {tone}>{tone}</Badge>{/each}
		<Badge variant="outline">outline</Badge>
		<Badge dot tone="success">with dot</Badge>
		<Badge icon={Sparkles} tone="agent">Opus 5.5</Badge>
	</div>
	<div class="row rail-sample">
		{#each OUTCOMES as outcome (outcome)}
			<span class="rail-node"><RailNode {outcome} size={18} /><code>{outcome}</code></span>
		{/each}
	</div>
	<div class="stack">
		<Callout tone="info" title="Information"
			>Something worth knowing about the content around it.</Callout
		>
		<Callout tone="success">Connected. The previous file was backed up.</Callout>
		<Callout tone="warning" title="Possible secret in this prompt"
			>Hide it so it is not kept.</Callout
		>
		<Callout tone="danger" title="The turn ended on an error">rate_limit_error</Callout>
	</div>
</Panel>

<style>
	.rail-sample {
		gap: var(--space-6);
	}

	.rail-node {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		font-size: var(--text-xs);
	}
</style>
