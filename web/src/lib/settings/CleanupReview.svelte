<script lang="ts">
	import { Trash2 } from '@lucide/svelte';
	import type { CleanupReport } from '#lib/api/settings.ts';
	import { integer } from '#lib/shared/format.ts';
	import { bytes, reason, summary } from '#lib/settings/describe.ts';
	import Button from '#lib/ui/Button.svelte';

	/**
	 * The dry run of a destructive action: what would be deleted, kind by kind, then confirm or
	 * cancel (the same two steps as Connect on the Connectors page).
	 */
	let {
		report,
		title,
		confirmLabel,
		allowEmpty = false,
		busy = false,
		onconfirm,
		oncancel
	}: {
		report: CleanupReport;
		title: string;
		confirmLabel: string;
		/** Confirm even when nothing would be deleted (VACUUM still frees space). */
		allowEmpty?: boolean;
		busy?: boolean;
		onconfirm: () => void;
		oncancel: () => void;
	} = $props();
</script>

<section class="review" aria-label={title}>
	<p class="title">{title}</p>
	<p>{summary(report)}</p>
	{#if report.removals.length}
		<div class="table-wrap">
			<table>
				<caption class="sr-only">{title}</caption>
				<thead>
					<tr>
						<th scope="col">Source</th>
						<th scope="col">Event</th>
						<th scope="col" class="n">Rows</th>
						<th scope="col" class="n">Size</th>
						<th scope="col">Why</th>
					</tr>
				</thead>
				<tbody>
					{#each report.removals as r (`${r.source}/${r.event}`)}
						<tr>
							<td class="mono">{r.source}</td>
							<td class="mono">{r.event ?? '(none)'}</td>
							<td class="n num">{integer(r.rows)}</td>
							<td class="n num">{bytes(r.bytes)}</td>
							<td class="muted">{reason(r.why)}</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
		<p class="muted note">This can't be undone. Prompts and your notes are never deleted.</p>
	{/if}
	<div class="actions">
		<Button
			variant="danger"
			icon={Trash2}
			onclick={onconfirm}
			disabled={!report.rows && !allowEmpty}
			loading={busy}>{confirmLabel}</Button
		>
		<Button variant="ghost" onclick={oncancel} disabled={busy}>Cancel</Button>
	</div>
</section>

<style>
	.review {
		display: grid;
		gap: var(--space-3);
		padding: var(--space-4);
		border-radius: var(--radius-md);
		background: var(--color-surface-sunken);
	}

	.title {
		font-weight: var(--weight-semibold);
	}

	.table-wrap {
		overflow-x: auto;
	}

	table {
		width: 100%;
		font-size: var(--text-sm);
	}

	th {
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
		font-weight: var(--weight-medium);
		text-align: left;
	}

	th,
	td {
		padding: var(--space-1_5) var(--space-2);
		border-bottom: var(--border-hairline) solid var(--color-border-subtle);
	}

	.n {
		text-align: right;
	}

	.mono {
		font-size: var(--text-xs);
	}

	.note {
		font-size: var(--text-xs);
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}
</style>
