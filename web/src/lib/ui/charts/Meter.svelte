<script lang="ts">
	import { percent } from '#lib/shared/format.ts';

	/**
	 * One ratio against its limit. The unfilled track is a lighter step of the fill's own hue, so the
	 * whole bar reads as one measure.
	 */
	let {
		value,
		label,
		color = 'var(--color-accent)',
		valueText
	}: {
		/** 0..1 */
		value: number;
		label: string;
		color?: string;
		/** What a screen reader hears; defaults to the percentage. */
		valueText?: string;
	} = $props();

	const clamped = $derived(Math.min(1, Math.max(0, value || 0)));
</script>

<div
	class="meter"
	role="meter"
	aria-label={label}
	aria-valuemin={0}
	aria-valuemax={100}
	aria-valuenow={Math.round(clamped * 100)}
	aria-valuetext={valueText ?? percent(clamped)}
	style:--fill={color}
>
	<span class="bar" style:width="{clamped * 100}%"></span>
</div>

<style>
	.meter {
		height: 6px;
		overflow: hidden;
		border-radius: var(--radius-full);
		background: color-mix(in srgb, var(--fill) 18%, transparent);
	}

	.bar {
		display: block;
		height: 100%;
		border-radius: var(--radius-full);
		background: var(--fill);
		transition: width var(--duration-slow) var(--ease-out);
	}
</style>
