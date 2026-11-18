<script lang="ts">
	import { ShieldCheck } from '@lucide/svelte';
	import type { LeakOut } from '#lib/api/types.ts';
	import { hiddenOnText, placeText } from '#lib/leaks/leaks.ts';

	/** A value you hid, as one quiet line: "Hidden on 16 Nov · API key in your prompt". */
	let { leak, now }: { leak: LeakOut; now?: Date } = $props();
</script>

<p class="hidden">
	<ShieldCheck size={14} aria-hidden="true" />
	<span>{hiddenOnText(leak, now)}</span>
	<span class="muted">· {leak.kind} {placeText(leak.place)}</span>
</p>

<style>
	.hidden {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1_5);
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}
</style>
