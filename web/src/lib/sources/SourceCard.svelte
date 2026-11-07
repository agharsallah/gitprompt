<script lang="ts">
	import { Activity, FileCog, Plug, RefreshCw, ShieldCheck, Unplug } from '@lucide/svelte';
	import { refreshAll } from '$app/navigation';
	import { api } from '#lib/api/client.ts';
	import type { Change, ConfigAction, Source } from '#lib/api/types.ts';
	import { clock } from '#lib/shared/clock.svelte.ts';
	import { compact, integer, relativePath } from '#lib/shared/format.ts';
	import { Task } from '#lib/shared/task.svelte.ts';
	import Badge from '#lib/ui/Badge.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import CodeBlock from '#lib/ui/CodeBlock.svelte';
	import RelativeTime from '#lib/ui/RelativeTime.svelte';

	let { source }: { source: Source } = $props();

	// A connect/disconnect is two steps: preview the exact file diff, then confirm it.
	let preview = $state.raw<{ action: ConfigAction; change: Change } | null>(null);
	let done = $state('');
	const task = new Task();
	const busy = $derived(task.busy);
	const blocked = $derived(source.blocks.length > 0);
	const telemetry = $derived(source.id.endsWith('-otel'));

	const status = $derived(
		blocked
			? { label: 'Blocked by policy', tone: 'danger' as const }
			: source.connected
				? source.outdated
					? { label: 'Update available', tone: 'warning' as const }
					: { label: 'Connected', tone: 'success' as const }
				: source.detected
					? { label: 'Not connected', tone: 'neutral' as const }
					: { label: 'Not installed', tone: 'neutral' as const }
	);

	/** Is anything arriving? Fresh within the hour, quiet within a day, else stale. */
	const freshness = $derived.by(() => {
		if (!source.last_at) return 'none';
		const age = clock.now - Date.parse(source.last_at);
		return age < 3_600_000 ? 'fresh' : age < 86_400_000 ? 'quiet' : 'stale';
	});

	const review = (action: ConfigAction) =>
		task.run(async () => {
			done = '';
			preview = { action, change: await api.configure(source.id, action, true) };
		});

	const confirm = (action: ConfigAction) =>
		task.run(async () => {
			const change = await api.configure(source.id, action);
			preview = null;
			done =
				action === 'connect'
					? `Connected.${change.backup ? ` The previous file was backed up to ${relativePath(change.backup)}.` : ''}`
					: 'Disconnected: only what GitPrompt added was removed.';
			await refreshAll();
		});

	const remap = () =>
		task.run(async () => {
			const { updated } = await api.remap(source.id);
			done = `Re-mapped stored events: ${integer(updated)} updated.`;
			await refreshAll();
		});
</script>

<article class="source" aria-labelledby="source-{source.id}">
	<header>
		<span class="icon" aria-hidden="true">
			{#if telemetry}<Activity size={18} />{:else}<Plug size={18} />{/if}
		</span>
		<div class="titles">
			<h2 id="source-{source.id}">{source.name}</h2>
			<p class="version muted">
				{source.version ? `Version ${source.version}` : 'Version unknown'}
			</p>
		</div>
		<Badge tone={status.tone} dot>{status.label}</Badge>
	</header>

	<dl class="health">
		<div>
			<dt>Last event</dt>
			<dd>
				<span class="fresh is-{freshness}" aria-hidden="true"></span><RelativeTime
					iso={source.last_at}
				/>
			</dd>
		</div>
		{#if !telemetry}<div>
				<dt>Prompts</dt>
				<dd>{integer(source.total)}</dd>
			</div>{/if}
		<div>
			<dt>Events</dt>
			<dd>{compact(source.events)}</dd>
		</div>
	</dl>

	<p class="terms"><ShieldCheck size={16} aria-hidden="true" /><span>{source.terms}</span></p>
	<p class="config">
		<FileCog size={14} aria-hidden="true" /><span class="mono"
			>{relativePath(source.config_file)}</span
		>
	</p>

	{#each source.blocks as block (block)}
		<Callout tone="danger" title="Can't connect">{block}. GitPrompt won't change that.</Callout>
	{/each}
	{#each source.warnings as warning (warning)}
		<Callout tone="warning">{warning}</Callout>
	{/each}
	{#if source.unmapped}
		<Callout tone="warning" title="{integer(source.unmapped)} events had no prompt">
			The tool may have changed its payload. Update the source spec, then re-map what is stored.
			{#snippet actions()}
				<Button size="sm" icon={RefreshCw} onclick={remap} loading={busy}
					>Re-map stored events</Button
				>
			{/snippet}
		</Callout>
	{/if}
	{#if source.connected && source.outdated && !preview}
		<Callout tone="info" title="GitPrompt can capture more from this source">
			Your connection predates the current spec. Review the change to update it.
		</Callout>
	{/if}

	{#if preview}
		<section class="review" aria-label="Review the change">
			<p class="review-title">
				{#if preview.change.diff}
					Review the change to <span class="mono">{relativePath(preview.change.file)}</span>
				{:else}
					Nothing to change in <span class="mono">{relativePath(preview.change.file)}</span>
				{/if}
			</p>
			{#if preview.change.diff}
				<CodeBlock
					code={preview.change.diff}
					diff
					label={relativePath(preview.change.file)}
					maxHeight="22rem"
				/>
				<p class="muted note">Lines marked + are added, − removed. A backup of the file is kept.</p>
			{/if}
			<div class="actions">
				<Button
					variant={preview.action === 'disconnect' ? 'danger' : 'primary'}
					onclick={() => confirm(preview!.action)}
					disabled={!preview.change.diff}
					loading={busy}
				>
					{preview.action === 'connect'
						? source.connected
							? 'Update connection'
							: 'Connect'
						: 'Disconnect'}
				</Button>
				<Button variant="ghost" onclick={() => (preview = null)} disabled={busy}>Cancel</Button>
			</div>
		</section>
	{:else}
		<div class="actions">
			{#if source.connected}
				{#if source.outdated}
					<Button
						variant="primary"
						onclick={() => review('connect')}
						disabled={blocked}
						loading={busy}
					>
						Review the update
					</Button>
				{/if}
				<Button
					icon={Unplug}
					onclick={() => review('disconnect')}
					loading={busy && !source.outdated}>Disconnect</Button
				>
			{:else}
				<Button
					variant="primary"
					icon={Plug}
					onclick={() => review('connect')}
					disabled={!source.detected || blocked}
					loading={busy}
				>
					Review and connect
				</Button>
				{#if !source.detected}<span class="muted hint">Install the tool first.</span>{/if}
			{/if}
		</div>
	{/if}

	{#if done}<Callout tone="success" live>{done}</Callout>{/if}
	{#if task.error}<Callout tone="danger" live>{task.error}</Callout>{/if}
</article>

<style>
	.source {
		display: grid;
		gap: var(--space-4);
		padding: var(--space-5);
		border-radius: var(--radius-lg);
		background: var(--color-surface);
		box-shadow: inset 0 0 0 1px var(--color-border);
	}

	header {
		display: flex;
		align-items: center;
		gap: var(--space-3);
	}

	.icon {
		display: grid;
		place-items: center;
		width: 2.25rem;
		height: 2.25rem;
		border-radius: var(--radius-md);
		background: var(--color-accent-soft);
		color: var(--color-accent);
	}

	.titles {
		flex: 1;
		min-width: 0;
	}

	h2 {
		font-size: var(--text-lg);
	}

	.version {
		font-size: var(--text-xs);
	}

	.health {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-3) var(--space-8);
		margin: 0;
	}

	.health div {
		display: grid;
	}

	dt {
		color: var(--color-text-secondary);
		font-size: var(--text-xs);
	}

	dd {
		display: flex;
		align-items: center;
		gap: var(--space-1_5);
		margin: 0;
		font-size: var(--text-lg);
		font-weight: var(--weight-semibold);
	}

	.fresh {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--color-text-muted);
	}

	.fresh.is-fresh {
		background: var(--color-success-solid);
	}

	.fresh.is-quiet {
		background: var(--color-warning-solid);
	}

	.terms,
	.config {
		display: flex;
		gap: var(--space-2);
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}

	.terms {
		max-width: var(--reading-max);
	}

	.terms :global(svg),
	.config :global(svg) {
		flex: none;
		margin-top: 2px;
		color: var(--color-text-muted);
	}

	.config .mono {
		font-size: var(--text-xs);
		overflow-wrap: anywhere;
	}

	.review {
		display: grid;
		gap: var(--space-3);
		padding: var(--space-4);
		border-radius: var(--radius-md);
		background: var(--color-surface-sunken);
	}

	.review-title {
		font-weight: var(--weight-semibold);
	}

	.review-title .mono {
		font-size: var(--text-sm);
		font-weight: var(--weight-regular);
	}

	.note {
		font-size: var(--text-xs);
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
	}

	.hint {
		font-size: var(--text-sm);
	}
</style>
