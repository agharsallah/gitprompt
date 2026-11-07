<script lang="ts">
	import type { Prompt } from '#lib/api/types.ts';
	import { originOf } from '#lib/prompts/origin.ts';
	import Badge from '#lib/ui/Badge.svelte';

	/** Who produced a prompt, as an icon and a label. Nothing for prompts you typed. */
	let { prompt, size = 'sm' }: { prompt: Pick<Prompt, 'origin'>; size?: 'sm' | 'md' } = $props();

	const info = $derived(originOf(prompt));
</script>

{#if info.system}
	<Badge variant="outline" icon={info.icon} title={info.hint} {size}>{info.label}</Badge>
{/if}
