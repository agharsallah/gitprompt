<script lang="ts">
	import { untrack } from 'svelte';
	import { api } from '#lib/api/client.ts';
	import type { Source } from '#lib/api/types.ts';
	import { relativePath } from '#lib/shared/format.ts';
	import { Task } from '#lib/shared/task.svelte.ts';
	import { CAPABILITIES, CAPABILITY_IDS } from '#lib/sources/capabilities.ts';
	import Badge from '#lib/ui/Badge.svelte';
	import Callout from '#lib/ui/Callout.svelte';
	import CodeBlock from '#lib/ui/CodeBlock.svelte';
	import Disclosure from '#lib/ui/Disclosure.svelte';

	/**
	 * The spec a connector runs on: which file (bundled or your override), what it yields, how its
	 * events find their prompt, and the file itself, read when you open it.
	 */
	let { source }: { source: Source } = $props();

	let open = $state(false);
	let text = $state<string | null>(null);
	const task = new Task();

	const yields = $derived(CAPABILITY_IDS.filter((c) => source.capabilities.includes(c)));
	const file = $derived(source.spec);
	const where = $derived(
		file.origin === 'user'
			? file.overrides
				? 'Your override of the bundled spec'
				: 'Your own spec'
			: 'Bundled with GitPrompt'
	);

	// Read the file the first time the section opens (and again after an error, on the next open).
	const load = () =>
		task.run(async () => {
			text = (await api.spec(source.id)).text;
		});
	$effect(() => {
		if (open) untrack(() => text === null && !task.busy && load());
	});
</script>

<Disclosure summary="Spec in use" bind:open>
	{#snippet meta()}{file.origin === 'user' ? 'Your file' : 'Bundled'}{/snippet}
	<div class="spec">
		<dl>
			<dt>File</dt>
			<dd>
				{where}: <span class="mono path">{relativePath(file.path)}</span>
			</dd>
			<dt>Yields</dt>
			<dd>
				{#if yields.length}
					<ul class="caps" aria-label="What this connector yields">
						{#each yields as c (c)}<li>
								<Badge size="sm" variant="outline">{CAPABILITIES[c].label}</Badge>
							</li>{/each}
					</ul>
				{:else}
					<span class="muted">Not declared in the spec.</span>
				{/if}
			</dd>
			<dt>Turns</dt>
			<dd>
				{source.join === 'session'
					? 'The tool sends no prompt id: each event joins the latest prompt of its session.'
					: 'Events join their prompt by the prompt id the tool sends.'}
			</dd>
		</dl>
		{#if task.error}<Callout tone="danger" live>{task.error}</Callout>{/if}
		{#if text !== null}<CodeBlock code={text} label={file.name} maxHeight="20rem" />{/if}
		<p class="muted">
			To change it, copy the file to <code>~/.gitprompt/sources/</code> and edit the copy; it replaces
			the bundled one, and a broken copy falls back to it.
		</p>
	</div>
</Disclosure>

<style>
	.spec {
		display: grid;
		gap: var(--space-3);
		padding: var(--space-2) var(--space-2) var(--space-1);
		font-size: var(--text-sm);
	}

	dl {
		display: grid;
		grid-template-columns: max-content minmax(0, 1fr);
		gap: var(--space-2) var(--space-3);
		margin: 0;
	}

	dt {
		color: var(--color-text-secondary);
		font-weight: var(--weight-medium);
	}

	dd {
		margin: 0;
		min-width: 0;
	}

	.path {
		font-size: var(--text-xs);
		overflow-wrap: anywhere;
	}

	.caps {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1);
	}

	@media (max-width: 30rem) {
		dl {
			grid-template-columns: minmax(0, 1fr);
		}
	}
</style>
