<script lang="ts">
	import { Share } from '@lucide/svelte';
	import type { Component } from 'svelte';
	import type { ExportTarget } from '#lib/export/export.ts';
	import { describeError } from '#lib/errors/describe.ts';
	import { registerCommand } from '#lib/palette/commands.ts';
	import { track } from '#lib/shared/usage.ts';
	import Button from '#lib/ui/Button.svelte';

	/** A page's one export entry point: opens the preview, which starts at the most private options. */
	let { target }: { target: ExportTarget } = $props();
	let open = $state(false);

	// The dialog's code is fetched when it is about to be needed (pointer over or focus on the
	// button), not with the page. A failed fetch is said next to the button.
	type DialogProps = { target: ExportTarget; open?: boolean };
	let Dialog = $state.raw<Component<DialogProps, Record<string, never>, 'open'> | null>(null);
	let pending: Promise<unknown> | null = null;
	let busy = $state(false);
	let failure = $state('');

	function prefetch() {
		pending ??= import('#lib/export/ExportDialog.svelte').then(
			(module) => (Dialog = module.default),
			(e) => {
				pending = null;
				throw e;
			}
		);
		return pending;
	}

	// The palette's "Export this view" exists while this page has something to export.
	$effect(() =>
		registerCommand({
			id: 'export.open',
			title: 'Export this view',
			keywords: ['download', 'save', 'report', 'html', 'csv', 'markdown', 'json'],
			run: show
		})
	);

	async function show() {
		track('export.open');
		busy = true;
		failure = '';
		try {
			await prefetch();
			open = true;
		} catch (e) {
			console.error('GitPrompt: the export dialog could not be loaded', e);
			const d = describeError(e, undefined, 'the export dialog');
			failure = d.reload ? 'Export needs a reload of the page.' : `${d.title}.`;
		} finally {
			busy = false;
		}
	}
</script>

<Button
	icon={Share}
	loading={busy}
	onpointerenter={() => prefetch().catch(() => {})}
	onfocus={() => prefetch().catch(() => {})}
	onclick={show}>Export</Button
>
{#if failure}<span class="failure" role="alert">{failure}</span>{/if}
{#if open && Dialog}<Dialog {target} bind:open />{/if}

<style>
	.failure {
		color: var(--color-danger-text);
		font-size: var(--text-sm);
	}
</style>
