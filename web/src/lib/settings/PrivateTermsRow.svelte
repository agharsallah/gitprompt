<script lang="ts">
	import type { SettingsState } from '#lib/api/settings.ts';
	import type { PreferenceEditor } from '#lib/settings/editor.svelte.ts';
	import ListSettingRow from '#lib/settings/ListSettingRow.svelte';

	/** Your own private words (`private_terms`): each one in a prompt or an answer is a possible leak. */
	let { prefs, editor }: { prefs: SettingsState; editor: PreferenceEditor } = $props();
</script>

<ListSettingRow
	label="Your own private words"
	description="Words GitPrompt flags as a possible leak wherever they appear, as whole words in any case (a client's name, a project's code name). Changing the list checks every stored prompt again."
	setting={prefs.settings.private_terms}
	file={prefs.file}
	fieldLabel="Private words, separated by commas"
	placeholder="Project Falcon, acme-internal"
	busy={editor.task.busy}
	save={(values) => void editor.set({ private_terms: values })}
/>
