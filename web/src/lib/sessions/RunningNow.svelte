<script lang="ts">
	import { live, type StreamState } from '#lib/api/live.svelte.ts';
	import { clock } from '#lib/shared/clock.svelte.ts';
	import { eventAge, presence, runningLine, type Activity } from '#lib/sessions/running.ts';
	import Badge from '#lib/ui/Badge.svelte';

	/**
	 * "Running · last event 12 s ago" while a session is still going, nothing once you left it.
	 * Follows the shared clock and the live stream; the time and the stream state can be given
	 * instead (samples).
	 */
	let {
		lastEventAt,
		ended,
		now,
		stream
	}: Activity & {
		/** A fixed "now"; default: the shared clock. */ now?: number;
		/** A fixed stream state; default: the live stream. */ stream?: StreamState;
	} = $props();

	const at = $derived(now ?? clock.now);
	const going = $derived(presence({ lastEventAt, ended }, at, stream ?? live.state) === 'running');
	const age = $derived(eventAge(lastEventAt, at) ?? 0);
</script>

{#if going}
	<Badge tone="success" dot size="sm">{runningLine(age)}</Badge>
{/if}
