<script lang="ts">
	import { Eye, EyeOff } from '@lucide/svelte';
	import type { SettingsState } from '#lib/api/settings.ts';
	import { PreferenceEditor } from '#lib/settings/editor.svelte.ts';
	import SettingRow from '#lib/settings/SettingRow.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import Panel from '#lib/ui/Panel.svelte';
	import SegmentedControl from '#lib/ui/SegmentedControl.svelte';

	let { prefs }: { prefs: SettingsState } = $props();

	const editor = new PreferenceEditor(() => prefs);
	const hide = $derived(prefs.settings.hide_secrets_on_ingest);
	const OPTIONS = [
		{ value: 'keep' as const, label: 'Store as received', icon: Eye },
		{ value: 'hide' as const, label: 'Hide secrets', icon: EyeOff }
	];
</script>

<Panel
	title="Privacy"
	description="Everything stays on this machine; these settings limit what is stored at all."
>
	<SettingRow
		label="Detected secrets"
		description="API keys, tokens, private keys and password assignments in a prompt or event can be replaced by “[hidden …]” before anything is stored. Prompts stored earlier keep their text: use Hide secrets on a prompt to clean one."
		setting={hide}
		file={prefs.file}
		value={hide.value ? 'Hidden on arrival' : 'Stored as received'}
	>
		<SegmentedControl
			label="Detected secrets"
			bind:value={
				() => (editor.value('hide_secrets_on_ingest') ? 'hide' : 'keep'),
				(v) => editor.set({ hide_secrets_on_ingest: v === 'hide' })
			}
			options={OPTIONS}
		/>
	</SettingRow>

	<p class="muted note">
		Export options (removing absolute paths and email addresses from what you share) come with Share
		and export.
	</p>

	{#if editor.message}<Callout tone="success" live>{editor.message}</Callout>{/if}
	{#if editor.task.error}<Callout tone="danger" live>{editor.task.error}</Callout>{/if}
</Panel>

<style>
	.note {
		margin-top: var(--space-2);
		font-size: var(--text-sm);
	}
</style>
