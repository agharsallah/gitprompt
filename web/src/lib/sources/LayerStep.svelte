<script lang="ts">
	import {
		ArrowRight,
		Check,
		CircleFadingArrowUp,
		Plug,
		RefreshCw,
		ShieldCheck,
		Unplug,
		type LucideIcon
	} from '@lucide/svelte';
	import { onMount, tick } from 'svelte';
	import { refreshAll } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { api } from '#lib/api/client.ts';
	import type { Change, ConfigAction, Source, StatsBucket } from '#lib/api/types.ts';
	import { clock } from '#lib/shared/clock.svelte.ts';
	import { integer, relativePath } from '#lib/shared/format.ts';
	import { Task } from '#lib/shared/task.svelte.ts';
	import Collects from '#lib/sources/Collects.svelte';
	import ConsentReview from '#lib/sources/ConsentReview.svelte';
	import Heartbeat from '#lib/sources/Heartbeat.svelte';
	import OriginCounts from '#lib/sources/OriginCounts.svelte';
	import StateBadge from '#lib/sources/StateBadge.svelte';
	import {
		layerName,
		present,
		promptsQuery,
		type ConnectorAction,
		type ConnectorStateId
	} from '#lib/sources/state.ts';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';

	/**
	 * One layer of a tool (its hooks, its telemetry) as a step: its state, the one action it needs
	 * now, and the consent review when that action changes the tool's settings. After a change,
	 * focus returns to the layer's heading and the result is announced.
	 */
	let {
		source,
		step,
		optional = false,
		origins,
		autoReview = false
	}: {
		source: Source;
		step: number;
		optional?: boolean;
		origins?: StatsBucket[];
		/** Open the consent review on arrival (first run: /sources?connect=<id>). */
		autoReview?: boolean;
	} = $props();

	const current = $derived(present(source, clock.now));
	let preview = $state.raw<{ action: ConfigAction; change: Change } | null>(null);
	let done = $state('');
	let heading = $state<HTMLElement>();
	const task = new Task();
	const id = $props.id();

	const ICONS: Record<ConnectorAction, LucideIcon> = {
		connect: Plug,
		update: CircleFadingArrowUp,
		remap: RefreshCw,
		recheck: RefreshCw,
		view: ArrowRight
	};

	const focusHeading = async () => {
		await tick();
		heading?.focus();
	};

	const review = (action: ConfigAction) =>
		task.run(async () => {
			done = '';
			preview = { action, change: await api.configure(source.id, action, true) };
		});

	const confirm = (action: ConfigAction) =>
		task.run(async () => {
			const change = await api.configure(source.id, action);
			const update = source.connected && action === 'connect';
			preview = null;
			await refreshAll();
			done =
				action === 'disconnect'
					? 'Disconnected: only what GitPrompt added was removed.'
					: `${update ? 'Updated' : 'Connected'}.${change.backup ? ` The previous file is backed up at ${relativePath(change.backup)}.` : ''}`;
			await focusHeading();
		});

	const cancel = () => {
		preview = null;
		focusHeading();
	};

	const recheck = () =>
		task.run(async () => {
			done = '';
			await refreshAll();
			done = `Checked again: ${current.label.toLowerCase()}.`;
		});

	const remap = () =>
		task.run(async () => {
			const { updated } = await api.remap(source.id);
			await refreshAll();
			done = `Re-mapped stored events: ${integer(updated)} updated.`;
		});

	const ACTIONS: Record<Exclude<ConnectorAction, 'view'>, () => unknown> = {
		connect: () => review('connect'),
		update: () => review('connect'),
		recheck,
		remap
	};

	// The moment capture starts working is worth saying out loud.
	let seen: ConnectorStateId | null = null;
	$effect(() => {
		if (seen === 'waiting' && current.id === 'receiving')
			done = 'First event received: capture works.';
		seen = current.id;
	});

	onMount(() => {
		if (autoReview && (current.action === 'connect' || current.action === 'update'))
			review('connect');
	});
</script>

<li class={['layer', source.connected && 'connected']} aria-labelledby={id}>
	<span class="marker" aria-hidden="true">
		{#if source.connected}<Check size={14} />{:else}{step}{/if}
	</span>
	<div class="body">
		<header>
			<h3 {id} bind:this={heading} tabindex="-1">
				{layerName(source)}
				{#if optional}<span class="optional">Optional</span>{/if}
			</h3>
			<StateBadge info={current} />
		</header>
		{#if source.summary}<p class="summary">{source.summary}</p>{/if}
		<p class="explanation">{current.explanation}</p>

		{#if source.last_at}<Heartbeat {source} />{/if}
		{#if origins}<OriginCounts groups={origins} />{/if}

		{#each source.blocks as block (block)}
			<Callout tone="danger" title="Turned off by a setting"
				>{block}. GitPrompt won’t change that.</Callout
			>
		{/each}
		{#each source.warnings as warning (warning)}
			<Callout tone="warning">{warning}.</Callout>
		{/each}

		{#if preview}
			<ConsentReview
				change={preview.change}
				action={preview.action}
				update={source.connected}
				busy={task.busy}
				onconfirm={() => confirm(preview!.action)}
				oncancel={cancel}
			/>
		{:else}
			<div class="actions">
				{#if current.action === 'view'}
					<Button href="{resolve('/prompts')}{promptsQuery(source)}" icon={ICONS.view}
						>{current.actionLabel}</Button
					>
				{:else}
					<Button
						variant={current.action === 'recheck' ? 'secondary' : 'primary'}
						icon={ICONS[current.action]}
						onclick={ACTIONS[current.action]}
						loading={task.busy}>{current.actionLabel}</Button
					>
				{/if}
				{#if source.connected}
					<Button
						variant="ghost"
						icon={Unplug}
						onclick={() => review('disconnect')}
						disabled={task.busy}>Disconnect</Button
					>
				{/if}
			</div>
		{/if}

		{#if done}<Callout tone="success" live>{done}</Callout>{/if}
		{#if task.error}<Callout tone="danger" live>{task.error}</Callout>{/if}

		{#if !source.connected && source.terms}
			<p class="terms"><ShieldCheck size={16} aria-hidden="true" /><span>{source.terms}</span></p>
		{/if}
		<Collects {source} />
	</div>
</li>

<style>
	.layer {
		position: relative;
		display: grid;
		grid-template-columns: 1.75rem minmax(0, 1fr);
		gap: var(--space-3);
	}

	/* Layers sit on a rail, like everything else that happens in order here. */
	.layer:not(:last-child)::after {
		content: '';
		position: absolute;
		top: 2rem;
		bottom: calc(-1 * var(--space-5) + 0.25rem);
		left: calc(0.875rem - 1px);
		width: 2px;
		background: var(--color-border);
	}

	.marker {
		display: grid;
		place-items: center;
		width: 1.75rem;
		height: 1.75rem;
		border: 2px solid var(--color-border-strong);
		border-radius: 50%;
		background: var(--color-surface);
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
		font-weight: var(--weight-semibold);
	}

	.connected .marker {
		border-color: var(--color-accent);
		background: var(--color-accent);
		color: var(--color-text-on-accent);
	}

	.body {
		display: grid;
		gap: var(--space-3);
		min-width: 0;
	}

	header {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2) var(--space-3);
		min-height: 1.75rem;
	}

	h3 {
		display: flex;
		align-items: baseline;
		gap: var(--space-2);
		font-size: var(--text-md);
		outline-offset: 2px;
	}

	.optional {
		color: var(--color-text-muted);
		font-size: var(--text-xs);
		font-weight: var(--weight-medium);
	}

	.summary {
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}

	.explanation {
		max-width: var(--reading-max);
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
	}

	.terms {
		display: flex;
		gap: var(--space-2);
		max-width: var(--reading-max);
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}

	.terms :global(svg) {
		flex: none;
		margin-top: 2px;
		color: var(--color-text-muted);
	}
</style>
