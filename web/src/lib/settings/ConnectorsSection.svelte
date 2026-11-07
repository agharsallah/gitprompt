<script lang="ts">
	import { Plug } from '@lucide/svelte';
	import { resolve } from '$app/paths';
	import type { Source } from '#lib/api/types.ts';
	import Badge from '#lib/ui/Badge.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Panel from '#lib/ui/Panel.svelte';
	import RelativeTime from '#lib/ui/RelativeTime.svelte';

	/** A summary of each connector; connecting and disconnecting happen on the Sources page. */
	let { sources }: { sources: Source[] } = $props();

	const status = (s: Source) =>
		s.blocks.length
			? { label: 'Blocked by policy', tone: 'danger' as const }
			: s.connected
				? { label: 'Connected', tone: 'success' as const }
				: { label: s.detected ? 'Not connected' : 'Not installed', tone: 'neutral' as const };
</script>

<Panel
	title="Connectors"
	description="The tools GitPrompt captures from. Each change to a tool's settings is reviewed as a diff on the Sources page."
>
	{#snippet actions()}
		<Button href={resolve('/sources')} icon={Plug} size="sm">Manage connectors</Button>
	{/snippet}
	<ul class="list">
		{#each sources as s (s.id)}
			{@const st = status(s)}
			<li>
				<span class="name">{s.name}</span>
				<Badge size="sm" tone={st.tone} dot>{st.label}</Badge>
				<span class="muted last">Last event <RelativeTime iso={s.last_at} /></span>
			</li>
		{/each}
	</ul>
</Panel>

<style>
	.list {
		display: grid;
		gap: var(--space-2);
	}

	li {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2) var(--space-3);
	}

	.name {
		font-weight: var(--weight-medium);
	}

	.last {
		margin-left: auto;
		font-size: var(--text-sm);
	}
</style>
