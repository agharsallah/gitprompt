<script lang="ts">
	import { KeyRound } from '@lucide/svelte';
	import { refreshAll } from '$app/navigation';
	import { settingsApi, type Rotation, type Security } from '#lib/api/settings.ts';
	import { relativePath } from '#lib/shared/format.ts';
	import { Task } from '#lib/shared/task.svelte.ts';
	import SettingRow from '#lib/settings/SettingRow.svelte';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import CodeBlock from '#lib/ui/CodeBlock.svelte';
	import Panel from '#lib/ui/Panel.svelte';
	import RelativeTime from '#lib/ui/RelativeTime.svelte';

	let { security }: { security: Security } = $props();

	const task = new Task();
	let preview = $state.raw<Rotation | null>(null);
	let done = $state('');

	const review = () =>
		task.run(async () => {
			done = '';
			preview = await settingsApi.rotateToken();
		});

	const confirm = () =>
		task.run(async () => {
			const rotation = await settingsApi.rotateToken(false);
			preview = null;
			const n = rotation.reconnects.length;
			done = n
				? `New token in place; ${n === 1 ? 'one source was' : `${n} sources were`} reconnected. Backups of the changed files are listed under Data.`
				: 'New token in place. No source was connected, so no tool settings changed.';
			await refreshAll();
		});
</script>

<Panel
	title="Security"
	description="Tools send what they capture to the receiver with a secret token, so other programs on this machine can't post fake prompts."
>
	<SettingRow
		label="Receiver token"
		description="Rotate it if it may have been copied. GitPrompt writes the new token into every connected tool, after you review each change."
	>
		<p class="facts">
			<span class="mono" title={security.token_file}>{relativePath(security.token_file)}</span>
			<span class="muted">changed <RelativeTime iso={security.token_changed} /></span>
		</p>
	</SettingRow>

	{#if preview}
		<section class="review" aria-label="Review the token rotation">
			{#each preview.blocked as reason (reason)}
				<Callout tone="danger" title="Can't rotate the token">
					{reason}. Disconnect it on the Connectors page first; GitPrompt won't work around that.
				</Callout>
			{/each}
			{#if !preview.blocked.length}
				<p class="title">
					{preview.reconnects.length
						? 'These settings files will be updated with the new token:'
						: 'No source is connected: only the token file changes.'}
				</p>
				{#each preview.reconnects as r (r.source)}
					{#if r.change.diff}
						<CodeBlock
							code={r.change.diff}
							diff
							label="{r.name} · {relativePath(r.change.file)}"
							maxHeight="18rem"
						/>
					{/if}
				{/each}
				<Callout tone="warning">
					Sessions already running may keep sending the old token until you restart them.
				</Callout>
			{/if}
			<div class="actions">
				<Button
					variant="danger"
					icon={KeyRound}
					onclick={confirm}
					disabled={preview.blocked.length > 0}
					loading={task.busy}
					>{preview.reconnects.length ? 'Rotate and reconnect' : 'Rotate token'}</Button
				>
				<Button variant="ghost" onclick={() => (preview = null)} disabled={task.busy}>Cancel</Button
				>
			</div>
		</section>
	{:else}
		<div class="actions">
			<Button icon={KeyRound} onclick={review} loading={task.busy}>Review token rotation</Button>
			{#if security.connected.length}
				<span class="muted hint">Connected: {security.connected.join(', ')}</span>
			{/if}
		</div>
	{/if}

	{#if done}<Callout tone="success" live>{done}</Callout>{/if}
	{#if task.error}<Callout tone="danger" live>{task.error}</Callout>{/if}
</Panel>

<style>
	.facts {
		display: grid;
		justify-items: end;
		font-size: var(--text-sm);
	}

	.facts .mono {
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

	.title {
		font-weight: var(--weight-semibold);
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

	@media (max-width: 40rem) {
		.facts {
			justify-items: start;
		}
	}
</style>
