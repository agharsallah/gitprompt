<script lang="ts">
	import type { SettingsState } from '#lib/api/settings.ts';
	import { RANGES, type RangeId } from '#lib/insights/range.ts';
	import type { WeekStart } from '#lib/insights/week.ts';
	import ThemeToggle from '#lib/layout/ThemeToggle.svelte';
	import { PreferenceEditor } from '#lib/settings/editor.svelte.ts';
	import SettingRow from '#lib/settings/SettingRow.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import Panel from '#lib/ui/Panel.svelte';
	import SegmentedControl from '#lib/ui/SegmentedControl.svelte';

	let { prefs }: { prefs: SettingsState } = $props();

	const editor = new PreferenceEditor(() => prefs);
	const s = $derived(prefs.settings);
	const rangeLabel = (id: RangeId) => RANGES.find((r) => r.id === id)?.label ?? id;
	const WEEK = [
		{ value: 'monday' as const, label: 'Monday' },
		{ value: 'sunday' as const, label: 'Sunday' }
	];
</script>

<Panel title="General" description="How the dashboard looks and which view it opens with.">
	<SettingRow label="Color theme" description="Kept in this browser only.">
		<ThemeToggle compact={false} />
	</SettingRow>

	<SettingRow
		label="Default time range"
		description="The window Overview, Sessions and Projects show when the link doesn't name one."
		setting={s.default_range}
		file={prefs.file}
		value={rangeLabel(s.default_range.value)}
	>
		<SegmentedControl
			label="Default time range"
			bind:value={
				() => editor.value('default_range'), (v: RangeId) => editor.set({ default_range: v })
			}
			options={RANGES.map((r) => ({ value: r.id, label: r.label }))}
		/>
	</SettingRow>

	<SettingRow
		label="Week starts on"
		description="The first row of the weekday-by-hour heatmap on Overview."
		setting={s.week_start}
		file={prefs.file}
		value={WEEK.find((w) => w.value === s.week_start.value)?.label}
	>
		<SegmentedControl
			label="Week starts on"
			bind:value={() => editor.value('week_start'), (v: WeekStart) => editor.set({ week_start: v })}
			options={WEEK}
		/>
	</SettingRow>

	<p class="muted note">
		Times are shown in this browser's time zone; the daemon stores them in UTC.
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
