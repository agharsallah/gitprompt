<script lang="ts">
	import { Monitor, Moon, Sun } from '@lucide/svelte';
	import { theme } from '#lib/layout/theme.svelte.ts';
	import SegmentedControl from '#lib/ui/SegmentedControl.svelte';

	/** Icons only in the sidebar; with labels on the Settings page. */
	let { compact = true }: { compact?: boolean } = $props();
</script>

<SegmentedControl
	label="Color theme"
	size={compact ? 'sm' : 'md'}
	iconOnly={compact}
	bind:value={theme.value}
	options={[
		{ value: 'system', label: 'Match system', icon: Monitor },
		{ value: 'light', label: 'Light', icon: Sun },
		{ value: 'dark', label: 'Dark', icon: Moon }
	]}
/>
