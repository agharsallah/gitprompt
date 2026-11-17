<script lang="ts">
	import { onMount } from 'svelte';
	import type { Attachment } from 'svelte/attachments';
	import type { SettingsState } from '#lib/api/settings.ts';
	import { platform } from '#lib/platform/index.ts';
	import type { PreferenceEditor } from '#lib/settings/editor.svelte.ts';
	import { RecallShortcut } from '#lib/settings/recall.svelte.ts';
	import SettingRow from '#lib/settings/SettingRow.svelte';
	import { shortcutFromPress, shortcutLabel, type Press } from '#lib/settings/shortcut.ts';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';

	/**
	 * Settings › General › the recall shortcut (desktop app only). It shows the stored shortcut, and
	 * "Change" waits for the key combination itself, so no one types `CmdOrCtrl+Alt+P`.
	 */
	let { prefs, editor }: { prefs: SettingsState; editor: PreferenceEditor } = $props();

	const mac = typeof navigator !== 'undefined' && /Mac|iPhone|iPad/i.test(navigator.platform);
	const setting = $derived(prefs.settings.recall_shortcut);
	const shown = $derived(shortcutLabel(editor.value('recall_shortcut'), mac));

	const recall = new RecallShortcut(
		{
			value: () => editor.value('recall_shortcut'),
			save: (shortcut) => editor.set({ recall_shortcut: shortcut })
		},
		platform.recall
	);
	onMount(() => void recall.load());

	const focusOnMount: Attachment<HTMLElement> = (element) => element.focus();

	function onkeydown(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			recall.recording = false;
			return;
		}
		event.preventDefault();
		const press: Press = event;
		const shortcut = shortcutFromPress(press, mac);
		if (shortcut) void recall.choose(shortcut);
	}
</script>

<SettingRow
	label="Recall shortcut"
	description="From any app, this shortcut opens a small window to find a prompt; Enter copies it and the window goes away. It works while the GitPrompt window is closed."
	{setting}
	file={prefs.file}
	value={shown}
>
	{#if recall.recording}
		<!-- A button that takes the keys (it is focused when it appears); blur or Esc gives up. -->
		<button
			class="recording"
			type="button"
			{@attach focusOnMount}
			{onkeydown}
			onblur={() => (recall.recording = false)}
		>
			Press the new shortcut…
		</button>
	{:else}
		<kbd>{shown}</kbd>
		<Button size="sm" onclick={() => (recall.recording = true)}>Change</Button>
	{/if}
</SettingRow>

{#if recall.problem}<Callout tone="warning" live>{recall.problem}</Callout>{/if}

<style>
	kbd {
		padding: var(--space-1) var(--space-2);
		border-radius: var(--radius-sm);
		background: var(--color-surface-sunken);
		box-shadow: inset 0 0 0 1px var(--color-border);
		font-family: var(--font-mono);
	}

	.recording {
		padding: var(--space-1) var(--space-3);
		border: var(--border-hairline) dashed var(--color-accent);
		border-radius: var(--radius-sm);
		background: none;
		color: var(--color-text-secondary);
	}
</style>
