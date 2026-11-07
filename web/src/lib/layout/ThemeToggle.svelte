<script lang="ts">
	import { Monitor, Moon, Sun } from '@lucide/svelte';
	import SegmentedControl from '#lib/ui/SegmentedControl.svelte';

	type Theme = 'system' | 'light' | 'dark';
	const KEY = 'gitprompt-theme';

	function stored(): Theme {
		try {
			const value = localStorage.getItem(KEY);
			return value === 'light' || value === 'dark' ? value : 'system';
		} catch {
			return 'system';
		}
	}

	let theme = $state<Theme>(stored());

	// The pinned theme lives on <html data-theme>; app.html applies it before first paint.
	$effect(() => {
		const root = document.documentElement;
		if (theme === 'system') delete root.dataset.theme;
		else root.dataset.theme = theme;
		try {
			if (theme === 'system') localStorage.removeItem(KEY);
			else localStorage.setItem(KEY, theme);
		} catch {
			// Storage can be unavailable (private windows); the choice then lasts for this visit.
		}
	});
</script>

<SegmentedControl
	label="Color theme"
	size="sm"
	iconOnly
	bind:value={theme}
	options={[
		{ value: 'system', label: 'Match system', icon: Monitor },
		{ value: 'light', label: 'Light', icon: Sun },
		{ value: 'dark', label: 'Dark', icon: Moon }
	]}
/>
