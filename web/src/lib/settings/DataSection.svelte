<script lang="ts">
	import { Archive, Eraser } from '@lucide/svelte';
	import { refreshAll } from '$app/navigation';
	import { settingsApi, type CleanupReport, type DataInfo } from '#lib/api/settings.ts';
	import { integer, plural, relativePath } from '#lib/shared/format.ts';
	import { Task } from '#lib/shared/task.svelte.ts';
	import { bytes, startOfDay, summary } from '#lib/settings/describe.ts';
	import CleanupReview from '#lib/settings/CleanupReview.svelte';
	import SettingRow from '#lib/settings/SettingRow.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import Disclosure from '#lib/ui/Disclosure.svelte';
	import Panel from '#lib/ui/Panel.svelte';
	import RelativeTime from '#lib/ui/RelativeTime.svelte';
	import SegmentedControl from '#lib/ui/SegmentedControl.svelte';
	import TextField from '#lib/ui/TextField.svelte';

	let { data, retentionDays }: { data: DataInfo; retentionDays: number | null } = $props();

	const task = new Task();
	let done = $state('');

	// Each destructive action is a dry run first; `confirm` applies exactly what was previewed.
	type Pending = {
		title: string;
		label: string;
		report: CleanupReport;
		/** Compact may confirm with nothing to delete: VACUUM alone still frees space. */
		allowEmpty: boolean;
		confirm: () => Promise<CleanupReport>;
	};
	let pending = $state.raw<Pending | null>(null);

	const reviewCompact = () =>
		task.run(async () => {
			done = '';
			pending = {
				title: 'Compact the database',
				label: 'Compact',
				report: await settingsApi.compact(retentionDays),
				allowEmpty: true,
				confirm: () => settingsApi.compact(retentionDays, false)
			};
		});

	let source = $state('all');
	let day = $state('');
	const before = $derived(startOfDay(day));
	const sourceOptions = $derived([
		{ value: 'all', label: 'All sources' },
		...data.events.map((e) => ({ value: e.source, label: e.source }))
	]);

	const reviewDelete = () =>
		task.run(async () => {
			done = '';
			const selection = { source: source === 'all' ? null : source, before };
			const report = await settingsApi.deleteEvents(selection);
			// Without a date the server fixes "until now"; confirming uses that same time.
			const fixed = { ...selection, before: report.removals[0]?.before ?? selection.before };
			pending = {
				title: 'Delete events',
				label: `Delete ${plural(report.rows, 'event')}`,
				report,
				allowEmpty: false,
				confirm: () => settingsApi.deleteEvents(fixed, false)
			};
		});

	const confirm = () =>
		task.run(async () => {
			const report = await pending!.confirm();
			pending = null;
			done = summary(report);
			await refreshAll();
		});
</script>

<Panel
	title="Data"
	description="Where GitPrompt keeps what it captured, and how to make it smaller."
>
	<dl class="facts">
		<div>
			<dt>Database</dt>
			<dd class="mono">{data.db_path}</dd>
		</div>
		<div>
			<dt>Size</dt>
			<dd class="num">{bytes(data.db_bytes)}</dd>
		</div>
		<div>
			<dt>Data folder</dt>
			<dd class="mono">{data.home}</dd>
		</div>
		<div>
			<dt>Receiver</dt>
			<dd class="mono">{data.receiver}</dd>
		</div>
	</dl>
	<p class="muted note">
		The folder and port are set with GITPROMPT_HOME and GITPROMPT_PORT when GitPrompt starts.
	</p>

	{#if pending}
		<CleanupReview
			report={pending.report}
			title={pending.title}
			confirmLabel={pending.label}
			allowEmpty={pending.allowEmpty}
			busy={task.busy}
			onconfirm={confirm}
			oncancel={() => (pending = null)}
		/>
	{:else}
		<SettingRow
			label="Compact"
			description="Delete stored events the source specs no longer keep{retentionDays
				? ` and events older than ${plural(retentionDays, 'day')}`
				: ''}, then VACUUM to give the space back to the disk."
		>
			<Button size="sm" icon={Archive} onclick={reviewCompact} loading={task.busy}
				>Review clean-up</Button
			>
		</SettingRow>

		<SettingRow
			label="Delete events"
			description="Delete the raw events of one source, or those received before a day. Prompts and notes are kept."
		>
			<form
				class="delete"
				onsubmit={(e) => {
					e.preventDefault();
					void reviewDelete();
				}}
			>
				{#if data.events.length}
					<SegmentedControl label="Source" size="sm" bind:value={source} options={sourceOptions} />
				{/if}
				<TextField label="Received before (optional)" type="date" size="sm" bind:value={day} />
				<Button
					type="submit"
					size="sm"
					icon={Eraser}
					disabled={source === 'all' && !before}
					loading={task.busy}>Review</Button
				>
			</form>
		</SettingRow>
	{/if}

	<div class="events">
		<p class="label">Stored events</p>
		<ul>
			{#each data.events as e (e.source)}
				<li>
					<span class="mono">{e.source}</span>
					<span class="num">{integer(e.rows)}</span>
					<span class="muted num">{bytes(e.bytes)}</span>
				</li>
			{:else}
				<li class="muted">None yet.</li>
			{/each}
		</ul>
	</div>

	<Disclosure summary="Backups of tool settings ({data.backups.length})">
		<p class="muted note">
			Taken before every connect, disconnect and token rotation. To restore one, copy it back over
			the tool's settings file.
		</p>
		<ul class="backups">
			{#each data.backups as b (b.path)}
				<li>
					<span class="mono" title={b.path}>{relativePath(b.path)}</span>
					<span class="muted num">{bytes(b.bytes)}</span>
					<span class="muted"><RelativeTime iso={b.modified} /></span>
				</li>
			{:else}
				<li class="muted">No backups yet.</li>
			{/each}
		</ul>
	</Disclosure>

	{#if done}<Callout tone="success" live>{done}</Callout>{/if}
	{#if task.error}<Callout tone="danger" live>{task.error}</Callout>{/if}
</Panel>

<style>
	.facts {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(100%, 16rem), 1fr));
		gap: var(--space-3) var(--space-6);
		margin: 0 0 var(--space-2);
	}

	dt,
	.label {
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
	}

	dd {
		margin: 0;
		overflow-wrap: anywhere;
	}

	dd.mono {
		font-size: var(--text-sm);
	}

	.note {
		margin-bottom: var(--space-3);
		font-size: var(--text-sm);
	}

	.delete {
		display: flex;
		flex-wrap: wrap;
		align-items: flex-end;
		justify-content: flex-end;
		gap: var(--space-2);
	}

	.events {
		display: grid;
		gap: var(--space-1);
		margin: var(--space-4) 0;
	}

	.events li,
	.backups li {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2) var(--space-4);
		font-size: var(--text-sm);
	}

	.events .mono,
	.backups .mono {
		flex: 1;
		min-width: 0;
		font-size: var(--text-xs);
		overflow-wrap: anywhere;
	}

	.backups {
		display: grid;
		gap: var(--space-1_5);
	}
</style>
