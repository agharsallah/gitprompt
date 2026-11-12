<script lang="ts">
	import type { PromptDetail } from '#lib/api/types.ts';
	import CodeBlock from '#lib/ui/CodeBlock.svelte';
	import KeyValue from '#lib/ui/KeyValue.svelte';
	import { capturedFields, unmappedPayload } from '#lib/prompts/detail.ts';

	/** The prompt as stored: the fields the spec mapped, what it left unmapped, and the raw payload. */
	let { prompt }: { prompt: PromptDetail } = $props();

	const captured = $derived(capturedFields(prompt.fields));
	const extra = $derived(unmappedPayload(prompt.raw, prompt.fields));
</script>

<div class="raw">
	<section>
		<h3>Captured fields</h3>
		<KeyValue data={captured} rawKeys />
	</section>
	{#if Object.keys(extra).length}
		<section>
			<h3>Other payload fields</h3>
			<KeyValue data={extra} rawKeys />
		</section>
	{/if}
	<CodeBlock code={JSON.stringify(prompt.raw, null, 2)} label="Raw payload" maxHeight="28rem" />
</div>

<style>
	.raw {
		display: grid;
		gap: var(--space-5);
	}

	h3 {
		margin-bottom: var(--space-2);
		font-size: var(--text-md);
	}
</style>
