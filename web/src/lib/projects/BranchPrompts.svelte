<script lang="ts">
	import { Copy, Eye } from '@lucide/svelte';
	import ExportPreview from '#lib/export/ExportPreview.svelte';
	import type { Preview } from '#lib/export/preview.ts';
	import { registerCommand } from '#lib/palette/commands.ts';
	import { copyBranch, loadBranch } from '#lib/projects/branch.ts';
	import { Task } from '#lib/shared/task.svelte.ts';
	import Button from '#lib/ui/Button.svelte';

	/**
	 * The prompts behind a branch as Markdown for its pull request: copy it, or preview it first
	 * (the preview is the block the copy writes). While on screen, the palette can copy it too.
	 */
	let { project, branch }: { project: string; branch: string } = $props();

	let previewing = $state(false);
	let preview = $state<Preview | null>(null);
	const task = new Task();

	const copy = () => void copyBranch(project, branch);

	$effect(() =>
		registerCommand({
			id: 'branch.copy',
			title: `Copy the prompts behind ${branch}`,
			keywords: ['branch', 'pull request', 'pr', 'markdown', 'commits'],
			run: copy
		})
	);

	// A new branch (or project) previews again; a newer request wins over a slower one.
	let request = 0;
	$effect(() => {
		if (!previewing) return;
		const mine = ++request;
		const [p, b] = [project, branch];
		void task.run(async () => {
			const file = await loadBranch(p, b);
			if (mine === request) preview = file;
		});
	});
</script>

<div class="branch">
	<p class="lead">Prompts behind <code>{branch}</code></p>
	<Button size="sm" icon={Copy} onclick={copy}>Copy prompts behind this branch</Button>
	<Button
		size="sm"
		variant="ghost"
		icon={Eye}
		pressed={previewing}
		onclick={() => (previewing = !previewing)}>Preview</Button
	>
</div>
{#if previewing}
	<div class="preview">
		<ExportPreview mode="md" {preview} busy={task.busy} error={task.error} />
	</div>
{/if}

<style>
	.branch {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
		margin-bottom: var(--space-4);
	}

	.lead {
		margin: 0;
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}

	.preview {
		display: flex;
		height: 22rem;
		margin-bottom: var(--space-4);
		overflow: hidden;
		border-radius: var(--radius-md);
		box-shadow: inset 0 0 0 1px var(--color-border);
	}
</style>
