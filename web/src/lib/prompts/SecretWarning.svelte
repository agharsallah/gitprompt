<script lang="ts">
	import { ShieldAlert } from '@lucide/svelte';
	import { api } from '#lib/api/client.ts';
	import { Task } from '#lib/shared/task.svelte.ts';
	import Button from '#lib/ui/Button.svelte';
	import Callout from '#lib/ui/Callout.svelte';

	/**
	 * The secrets detected in a prompt (their kinds, never the values), and the one way to act on
	 * them: replace them everywhere they were stored for the prompt, after a confirmation.
	 */
	let { id, secrets, onchange }: { id: number; secrets: string[]; onchange: () => void } = $props();

	const redaction = new Task();
	async function hide() {
		const question =
			'Replace the detected secrets in everything stored for this prompt? This cannot be undone.';
		if (confirm(question) && (await redaction.run(() => api.redact(id)))) onchange();
	}
</script>

<Callout tone="warning" icon={ShieldAlert} title="Possible secret in this prompt">
	Detected: {secrets.join(', ')}. The value is stored only on this machine; hiding it replaces it
	everywhere it was stored for this prompt.
	{#snippet actions()}
		<Button size="sm" variant="secondary" onclick={hide} loading={redaction.busy}
			>Hide the secret</Button
		>
	{/snippet}
</Callout>
{#if redaction.error}<Callout tone="danger" live>{redaction.error}</Callout>{/if}
