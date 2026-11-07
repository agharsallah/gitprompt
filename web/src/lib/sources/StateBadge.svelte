<script lang="ts">
	import type { StateInfo } from '#lib/sources/state.ts';
	import Badge from '#lib/ui/Badge.svelte';

	/** A connector's state as a badge: its icon and its label, in its tone (never colour alone). */
	let { info, size = 'md' }: { info: StateInfo; size?: 'sm' | 'md' } = $props();
</script>

<Badge tone={info.tone} icon={info.icon} {size}>{info.label}</Badge>
