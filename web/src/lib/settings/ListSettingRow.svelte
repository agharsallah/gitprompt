<script lang="ts">
	import type { Setting } from '#lib/api/settings.ts';
	import { allowSummary, allowText, parseAllow, sameAllow } from '#lib/settings/allow.ts';
	import SettingRow from '#lib/settings/SettingRow.svelte';
	import Button from '#lib/ui/Button.svelte';
	import TextField from '#lib/ui/TextField.svelte';

	/** A preference that is a list of words (`leak_allow`, `private_terms`), typed as one field. */
	let {
		label,
		description,
		setting,
		file,
		fieldLabel,
		placeholder,
		busy,
		save
	}: {
		label: string;
		description: string;
		setting: Setting<'leak_allow' | 'private_terms'>;
		file: string;
		/** What the field holds, for screen readers. */
		fieldLabel: string;
		placeholder: string;
		busy: boolean;
		save: (values: string[]) => void;
	} = $props();

	let draft = $derived(allowText(setting.value));
	const unchanged = $derived(sameAllow(draft, setting.value));
</script>

<SettingRow {label} {description} {setting} {file} value={allowSummary(setting.value)}>
	<form
		class="list"
		onsubmit={(e) => {
			e.preventDefault();
			if (!unchanged) save(parseAllow(draft));
		}}
	>
		<TextField label={fieldLabel} hideLabel size="sm" bind:value={draft} {placeholder} />
		<Button type="submit" size="sm" disabled={unchanged} loading={busy}>Save</Button>
	</form>
</SettingRow>

<style>
	.list {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
		min-width: min(22rem, 100%);
	}

	.list > :global(.field) {
		flex: 1;
	}
</style>
