<script lang="ts">
	import type { SettingsState } from '#lib/api/settings.ts';
	import type { PreferenceEditor } from '#lib/settings/editor.svelte.ts';
	import ListSettingRow from '#lib/settings/ListSettingRow.svelte';

	/** The values that are never a possible leak (`leak_allow`), typed as one comma-separated list. */
	let { prefs, editor }: { prefs: SettingsState; editor: PreferenceEditor } = $props();
</script>

<ListSettingRow
	label="Never a leak"
	description="Values GitPrompt should not flag (your other addresses, a shared test account). Your git email is never flagged."
	setting={prefs.settings.leak_allow}
	file={prefs.file}
	fieldLabel="Values never flagged, separated by commas"
	placeholder="me@work.example, test@example.com"
	busy={editor.task.busy}
	save={(values) => void editor.set({ leak_allow: values })}
/>
