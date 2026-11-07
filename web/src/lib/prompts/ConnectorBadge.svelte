<script lang="ts">
	import { Plug } from '@lucide/svelte';
	import type { Connector } from '#lib/api/types.ts';
	import Badge from '#lib/ui/Badge.svelte';

	/**
	 * The tool that sent a prompt: the plug icon (the same as the Tool filter) and its short name, in
	 * neutral ink. Tools are told apart by their name, never by colour: lagoon and iris stay reserved
	 * for you and the agent.
	 */
	let { connector, size = 'sm' }: { connector: Connector; size?: 'sm' | 'md' } = $props();
</script>

<Badge variant="outline" icon={Plug} {size} title="Sent by {connector.name}"
	>{connector.label}</Badge
>
