<script lang="ts">
	import type { SettingsState } from '#lib/api/settings.ts';
	import { platform } from '#lib/platform/index.ts';
	import { PreferenceEditor } from '#lib/settings/editor.svelte.ts';
	import {
		HOURS,
		hourLabel,
		LONG_TURN_MINUTES,
		minuteOptions,
		minutesLabel,
		QUIET_MINUTES,
		SWITCH,
		switchOf,
		type Switch,
		WEEKDAYS,
		weekdayLabel,
		weekdayOf
	} from '#lib/settings/notifications.ts';
	import SettingRow from '#lib/settings/SettingRow.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import Panel from '#lib/ui/Panel.svelte';
	import SegmentedControl from '#lib/ui/SegmentedControl.svelte';
	import Select from '#lib/ui/Select.svelte';

	let { prefs }: { prefs: SettingsState } = $props();

	const editor = new PreferenceEditor(() => prefs);
	const s = $derived(prefs.settings);
	const onOff = (on: boolean) => (on ? 'On' : 'Off');
</script>

<Panel
	title="Notifications"
	description="The daemon decides what deserves a notification, so it works with the window closed. Each kind notifies at most three times an hour."
>
	<SettingRow
		label="Long turns"
		description="When a turn that ran a long time finishes or fails. A turn you stopped is never notified."
		setting={s.notify_long_turn}
		file={prefs.file}
		value={onOff(s.notify_long_turn.value)}
	>
		<SegmentedControl
			label="Long turns"
			bind:value={
				() => switchOf(editor.value('notify_long_turn')),
				(v: Switch) => editor.set({ notify_long_turn: v === 'on' })
			}
			options={SWITCH}
		/>
	</SettingRow>

	{#if editor.value('notify_long_turn')}
		<SettingRow
			label="A turn is long after"
			setting={s.notify_long_turn_minutes}
			file={prefs.file}
			value={minutesLabel(s.notify_long_turn_minutes.value)}
		>
			<SegmentedControl
				label="A turn is long after"
				size="sm"
				bind:value={
					() => String(editor.value('notify_long_turn_minutes')),
					(v: string) => editor.set({ notify_long_turn_minutes: Number(v) })
				}
				options={minuteOptions(LONG_TURN_MINUTES, s.notify_long_turn_minutes.value)}
			/>
		</SettingRow>
	{/if}

	<SettingRow
		label="Possible leaks"
		description="When a key, a token or personal data turns up in something captured. Shown masked, never the value."
		setting={s.notify_leaks}
		file={prefs.file}
		value={onOff(s.notify_leaks.value)}
	>
		<SegmentedControl
			label="Possible leaks"
			bind:value={
				() => switchOf(editor.value('notify_leaks')),
				(v: Switch) => editor.set({ notify_leaks: v === 'on' })
			}
			options={SWITCH}
		/>
	</SettingRow>

	{#if platform.desktop}
		<SettingRow
			label="Dock badge"
			description="The number of possible leaks you have not hidden or dismissed yet, on the app's icon. It clears once each is handled."
			setting={s.leak_badge}
			file={prefs.file}
			value={onOff(s.leak_badge.value)}
		>
			<SegmentedControl
				label="Dock badge"
				bind:value={
					() => switchOf(editor.value('leak_badge')),
					(v: Switch) => editor.set({ leak_badge: v === 'on' })
				}
				options={SWITCH}
			/>
		</SettingRow>
	{/if}

	<SettingRow
		label="A connector gone quiet"
		description="When a connector that was sending stops while another connector of the same tool keeps sending (Claude Code's telemetry while its hooks still arrive)."
		setting={s.notify_quiet_connector}
		file={prefs.file}
		value={onOff(s.notify_quiet_connector.value)}
	>
		<SegmentedControl
			label="A connector gone quiet"
			bind:value={
				() => switchOf(editor.value('notify_quiet_connector')),
				(v: Switch) => editor.set({ notify_quiet_connector: v === 'on' })
			}
			options={SWITCH}
		/>
	</SettingRow>

	{#if editor.value('notify_quiet_connector')}
		<SettingRow
			label="Quiet after"
			setting={s.notify_quiet_connector_minutes}
			file={prefs.file}
			value={minutesLabel(s.notify_quiet_connector_minutes.value)}
		>
			<SegmentedControl
				label="Quiet after"
				size="sm"
				bind:value={
					() => String(editor.value('notify_quiet_connector_minutes')),
					(v: string) => editor.set({ notify_quiet_connector_minutes: Number(v) })
				}
				options={minuteOptions(QUIET_MINUTES, s.notify_quiet_connector_minutes.value)}
			/>
		</SettingRow>
	{/if}

	<SettingRow
		label="Weekly recap"
		description="Once a week: what the week cost, what was redone, what was committed and the one thing worth a look. Without the desktop app, it is a card on the Overview."
		setting={s.recap}
		file={prefs.file}
		value={onOff(s.recap.value)}
	>
		<SegmentedControl
			label="Weekly recap"
			bind:value={
				() => switchOf(editor.value('recap')), (v: Switch) => editor.set({ recap: v === 'on' })
			}
			options={SWITCH}
		/>
	</SettingRow>

	{#if editor.value('recap')}
		<SettingRow
			label="Recap day"
			setting={s.recap_weekday}
			file={prefs.file}
			value={weekdayLabel(s.recap_weekday.value)}
		>
			<Select
				label="Recap day"
				hideLabel
				options={WEEKDAYS}
				bind:value={
					() => editor.value('recap_weekday'),
					(v: string) => {
						const day = weekdayOf(v);
						if (day) void editor.set({ recap_weekday: day });
					}
				}
			/>
		</SettingRow>

		<SettingRow
			label="Recap hour"
			description="Local time."
			setting={s.recap_hour}
			file={prefs.file}
			value={hourLabel(s.recap_hour.value)}
		>
			<Select
				label="Recap hour"
				hideLabel
				options={HOURS}
				bind:value={
					() => String(editor.value('recap_hour')),
					(v: string) => editor.set({ recap_hour: Number(v) })
				}
			/>
		</SettingRow>
	{/if}

	{#if editor.message}<Callout tone="success" live>{editor.message}</Callout>{/if}
	{#if editor.task.error}<Callout tone="danger" live>{editor.task.error}</Callout>{/if}
</Panel>
