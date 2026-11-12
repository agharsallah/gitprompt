<script lang="ts">
	import {
		AtSign,
		Braces,
		CircleHelp,
		Link2,
		TextCursorInput,
		type LucideIcon
	} from '@lucide/svelte';
	import type { PromptDetail } from '#lib/api/types.ts';
	import CodeBlock from '#lib/ui/CodeBlock.svelte';
	import Disclosure from '#lib/ui/Disclosure.svelte';
	import { textFacts, type TextFact } from '#lib/prompts/detail.ts';
	import { injectedSummary, originOf, typedText } from '#lib/prompts/origin.ts';
	import Voice from '#lib/prompts/Voice.svelte';

	/**
	 * The prompt as you said it: your words (or the origin's, when the tool wrote it), the context the
	 * tool added around them, and what the text holds.
	 */
	let { prompt, headingId }: { prompt: PromptDetail; headingId: string } = $props();

	const ICONS: Record<TextFact['kind'], LucideIcon> = {
		size: TextCursorInput,
		questions: CircleHelp,
		code: Braces,
		links: Link2,
		mentions: AtSign
	};

	const origin = $derived(originOf(prompt));
	const text = $derived(typedText(prompt));
	const injected = $derived(injectedSummary(prompt.injected ?? []));
	const facts = $derived(textFacts(prompt.stats));
</script>

<Voice
	id={headingId}
	tone={origin.system ? 'system' : 'you'}
	text={text ?? 'No prompt text was captured: the source mapping may be outdated.'}
	moreLabel="Show the whole prompt"
	speaker={origin.system ? origin.label : 'You'}
	icon={origin.system ? origin.icon : undefined}
	title={origin.system ? origin.hint : undefined}
>
	{#if injected}
		<div class="injected">
			<Disclosure summary="Context added by the tool">
				{#snippet meta()}{injected}{/snippet}
				<CodeBlock code={prompt.prompt ?? ''} label="Prompt as received" maxHeight="20rem" />
			</Disclosure>
		</div>
	{/if}
	<ul class="text-facts" aria-label="About the prompt text">
		{#each facts as fact (fact.kind)}
			{@const Icon = ICONS[fact.kind]}
			<li><Icon size={13} aria-hidden="true" />{fact.text}</li>
		{/each}
	</ul>
</Voice>

<style>
	.injected {
		max-width: var(--reading-max);
		font-size: var(--text-sm);
	}

	.text-facts {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-1) var(--space-4);
		color: var(--color-text-muted);
		font-size: var(--text-xs);
	}

	.text-facts li {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
	}
</style>
