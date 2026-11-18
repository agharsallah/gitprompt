<script lang="ts">
	import type { SettingsState } from '#lib/api/settings.ts';
	import { allowSummary, allowText, parseAllow, sameAllow } from '#lib/settings/allow.ts';
	import type { PreferenceEditor } from '#lib/settings/editor.svelte.ts';
	import SettingRow from '#lib/settings/SettingRow.svelte';
	import Button from '#lib/ui/Button.svelte';
	import TextField from '#lib/ui/TextField.svelte';

	/** The values that are never a possible leak (`leak_allow`), typed as one comma-separated list. */
	let { prefs, editor }: { prefs: SettingsState; editor: PreferenceEditor } = $props();

	const allow = $derived(prefs.settings.leak_allow);
	let draft = $derived(allowText(allow.value));
	const unchanged = $derived(sameAllow(draft, allow.value));
</script>

<SettingRow
	label="Never a leak"
	description="Values GitPrompt should not flag (your other addresses, a shared test account). Your git email is never flagged."
	setting={allow}
	file={prefs.file}
	value={allowSummary(allow.value)}
>
	<form
		class="allow"
		onsubmit={(e) => {
			e.preventDefault();
			if (!unchanged) void editor.set({ leak_allow: parseAllow(draft) });
		}}
	>
		<TextField
			label="Values never flagged, separated by commas"
			hideLabel
			size="sm"
			bind:value={draft}
			placeholder="me@work.example, test@example.com"
		/>
		<Button type="submit" size="sm" disabled={unchanged} loading={editor.task.busy}>Save</Button>
	</form>
</SettingRow>

<style>
	.allow {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
		min-width: min(22rem, 100%);
	}

	.allow > :global(.field) {
		flex: 1;
	}
</style>
