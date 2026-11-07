<script lang="ts">
	import { refreshAll } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { settingsApi, type CleanupReport, type SettingsState } from '#lib/api/settings.ts';
	import { plural } from '#lib/shared/format.ts';
	import { daysAgo } from '#lib/settings/describe.ts';
	import { PreferenceEditor } from '#lib/settings/editor.svelte.ts';
	import CleanupReview from '#lib/settings/CleanupReview.svelte';
	import SettingRow from '#lib/settings/SettingRow.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import Panel from '#lib/ui/Panel.svelte';
	import TextField from '#lib/ui/TextField.svelte';

	let { prefs }: { prefs: SettingsState } = $props();

	const editor = new PreferenceEditor(() => prefs);
	const retention = $derived(prefs.settings.event_retention_days);
	const kept = $derived(
		retention.value
			? `Raw events older than ${plural(retention.value, 'day')} are deleted, at start-up and every 6 hours.`
			: 'Raw events are kept forever.'
	);

	// The draft: a number of days, previewed (dry run) before it is saved.
	let days = $state('');
	let preview = $state.raw<{ days: number; before: string; report: CleanupReport } | null>(null);
	const parsed = $derived(Number.parseInt(days, 10));
	const valid = $derived(Number.isInteger(parsed) && parsed >= 1 && parsed <= 3650);

	const review = () =>
		editor.task.run(async () => {
			const before = daysAgo(parsed);
			preview = {
				days: parsed,
				before,
				report: await settingsApi.deleteEvents({ source: null, before })
			};
		});

	async function confirm() {
		if (!preview) return;
		const { days: n, before, report } = preview;
		// Save first: if that fails nothing is deleted. Then apply it now, as previewed.
		if (!(await editor.set({ event_retention_days: n }))) return;
		if (report.rows) {
			await editor.task.run(async () => {
				await settingsApi.deleteEvents({ source: null, before }, false);
				await refreshAll();
			});
		}
		preview = null;
		days = '';
	}
</script>

<Panel
	title="Capture"
	description="What GitPrompt keeps of what it receives. Prompts and your notes are always kept."
>
	<SettingRow
		label="Event retention"
		description="Tool calls, turn ends and telemetry are stored as raw events. Older ones can be deleted to keep the database small; their prompts stay, but lose the tools, usage and timeline of that turn."
		setting={retention}
		file={prefs.file}
		value={retention.value ? plural(retention.value, 'day') : 'Keep forever'}
	>
		<div class="retention">
			<p class="current">{kept}</p>
			{#if !preview}
				<form
					class="draft"
					onsubmit={(e) => {
						e.preventDefault();
						if (valid) review();
					}}
				>
					<TextField
						label="Keep raw events for (days)"
						type="number"
						min="1"
						max="3650"
						inputmode="numeric"
						size="sm"
						bind:value={days}
						placeholder={retention.value ? String(retention.value) : '30'}
					/>
					<Button type="submit" size="sm" disabled={!valid} loading={editor.task.busy}
						>Review</Button
					>
					{#if retention.value}
						<Button
							size="sm"
							variant="ghost"
							onclick={() => editor.set({ event_retention_days: null })}
							disabled={editor.task.busy}>Keep forever</Button
						>
					{/if}
				</form>
			{/if}
		</div>
	</SettingRow>

	{#if preview}
		<CleanupReview
			report={preview.report}
			title="Keep raw events for {plural(preview.days, 'day')}"
			confirmLabel={preview.report.rows
				? `Save and delete ${plural(preview.report.rows, 'event')} now`
				: 'Save'}
			allowEmpty
			busy={editor.task.busy}
			onconfirm={confirm}
			oncancel={() => (preview = null)}
		/>
	{/if}

	<SettingRow
		label="Telemetry noise"
		description="Lifecycle events that no page uses (hook registered, plugin loaded…) are dropped on arrival by each source's spec. Already stored ones can be removed under Data › Compact."
	>
		<Button size="sm" variant="ghost" href={resolve('/fields')}>See what is dropped</Button>
	</SettingRow>

	{#if editor.message}<Callout tone="success" live>{editor.message}</Callout>{/if}
	{#if editor.task.error}<Callout tone="danger" live>{editor.task.error}</Callout>{/if}
</Panel>

<style>
	.retention {
		display: grid;
		gap: var(--space-2);
		justify-items: end;
	}

	.current {
		font-size: var(--text-sm);
		text-align: right;
	}

	.draft {
		display: flex;
		flex-wrap: wrap;
		align-items: flex-end;
		gap: var(--space-2);
	}

	@media (max-width: 40rem) {
		.retention {
			justify-items: start;
		}

		.current {
			text-align: left;
		}
	}
</style>
