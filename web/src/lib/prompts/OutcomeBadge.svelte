<script lang="ts">
	import { CircleCheck, CircleDashed, CircleStop, CircleX, LoaderCircle } from '@lucide/svelte';
	import Badge from '#lib/ui/Badge.svelte';
	import { OUTCOMES, type Outcome } from '#lib/insights/outcome.ts';

	let { outcome, size = 'md' }: { outcome: Outcome; size?: 'sm' | 'md' } = $props();

	const ICONS = {
		done: CircleCheck,
		failed: CircleX,
		interrupted: CircleStop,
		running: LoaderCircle,
		open: CircleDashed
	};
	const info = $derived(OUTCOMES[outcome]);
</script>

<Badge tone={info.tone} icon={ICONS[outcome]} title={info.hint} {size}>{info.label}</Badge>
