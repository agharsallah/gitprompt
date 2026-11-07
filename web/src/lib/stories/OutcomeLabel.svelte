<script lang="ts">
	import { CircleCheck, CircleDashed, CircleX, Hourglass, Redo2 } from '@lucide/svelte';
	import type { TurnOutcome } from '#lib/api/types.ts';
	import { TURN_OUTCOMES } from '#lib/insights/outcome.ts';
	import Badge from '#lib/ui/Badge.svelte';

	/** How a turn ended, in the daemon's words (superseded included), as an icon and a label. */
	let { outcome }: { outcome: TurnOutcome } = $props();

	const ICONS = {
		done: CircleCheck,
		failed: CircleX,
		superseded: Redo2,
		running: Hourglass,
		open: CircleDashed
	};
	const info = $derived(TURN_OUTCOMES[outcome]);
</script>

<Badge tone={info.tone} variant="plain" size="sm" icon={ICONS[outcome]} title={info.hint}
	>{info.label}</Badge
>
