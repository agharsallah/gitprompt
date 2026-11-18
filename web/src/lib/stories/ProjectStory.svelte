<script lang="ts">
	import type { ProjectStory } from '#lib/api/types.ts';
	import EmptyState from '#lib/ui/EmptyState.svelte';
	import Evolution from '#lib/stories/Evolution.svelte';
	import PhaseSection from '#lib/stories/PhaseSection.svelte';

	/**
	 * How a project got built, prompt by prompt: its phases, then each phase on a rail. The header
	 * and figures above it are ProjectHeader's.
	 */
	let { story }: { story: ProjectStory } = $props();
</script>

{#if story.phases.length}
	{#if story.phases.length > 1}<Evolution phases={story.phases} />{/if}
	<div class="phases">
		{#each story.phases as phase (phase.index)}<PhaseSection {phase} />{/each}
	</div>
{:else}
	<EmptyState title="You typed nothing in this project">
		Every prompt here was sent by the tool or other agents.
	</EmptyState>
{/if}

<style>
	.phases {
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		gap: var(--space-10);
		margin-top: var(--space-8);
	}
</style>
