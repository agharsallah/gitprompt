<script lang="ts">
	import { Share } from '@lucide/svelte';
	import type { ExportTarget } from '#lib/export/export.ts';
	import ExportDialog from '#lib/export/ExportDialog.svelte';
	import Button from '#lib/ui/Button.svelte';

	/** A page's one export entry point: opens the preview, which starts at the most private options. */
	let { target }: { target: ExportTarget } = $props();
	let open = $state(false);
</script>

<Button icon={Share} onclick={() => (open = true)}>Export</Button>
{#if open}<ExportDialog {target} bind:open />{/if}
