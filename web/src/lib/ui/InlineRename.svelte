<script lang="ts">
	import { tick, untrack, type Snippet } from 'svelte';
	import { Pencil } from '@lucide/svelte';
	import { Rename } from '#lib/ui/inlineRename.svelte.ts';

	/**
	 * A text you can rename in place: a button starts editing, Enter saves, Esc cancels and leaving the
	 * field saves. The new text shows at once and goes back if `onsave` rejects (with the reason).
	 * Knows nothing about what it renames; the state is lib/ui/inlineRename.svelte.ts.
	 */
	let {
		value,
		label,
		onsave,
		fallback,
		what,
		maxlength = 80,
		display
	}: {
		/** The text shown. */
		value: string;
		/** What the field is, for its label and the button ("Session name"). */
		label: string;
		/** Stores the new text (`null`: back to `fallback`); may answer the text as stored. */
		onsave: (next: string | null) => Promise<string | null | void>;
		/** What an emptied text goes back to; without it, emptying the text cancels. */
		fallback?: string;
		/** What an error calls the saved thing ("the session name"). */
		what?: string;
		maxlength?: number;
		/** Draws the text itself (a link); a pencil button then starts the editing. */
		display?: Snippet<[string]>;
	} = $props();

	const rename = new Rename({
		value: () => value,
		fallback: () => fallback,
		save: (next) => onsave(next),
		what: () => what
	});
	// A new value from the owner replaces what was typed before it.
	$effect.pre(() => {
		void value;
		untrack(() => rename.settle());
	});

	const id = $props.id();
	let trigger = $state<HTMLButtonElement>();

	const focusField = (field: HTMLInputElement) => {
		field.focus();
		field.select();
	};

	/** Enter and Esc end the editing from the keyboard: focus goes back to the button. */
	async function onkeydown(event: KeyboardEvent) {
		if (event.key === 'Enter') await rename.commit();
		else if (event.key === 'Escape') rename.cancel();
		else return;
		event.preventDefault();
		await tick();
		trigger?.focus();
	}
</script>

<span class="rename">
	{#if rename.editing}
		<label class="sr-only" for={id}>{label}</label>
		<input
			{id}
			class="field"
			type="text"
			bind:value={rename.draft}
			{maxlength}
			autocomplete="off"
			{onkeydown}
			onblur={() => rename.commit()}
			{@attach focusField}
		/>
	{:else if display}
		{@render display(rename.shown)}
		<button
			bind:this={trigger}
			type="button"
			class="edit"
			aria-label="Edit {label.toLowerCase()}"
			onclick={() => rename.start()}><Pencil size={14} aria-hidden="true" /></button
		>
	{:else}
		<button
			bind:this={trigger}
			type="button"
			class="text"
			title="Edit {label.toLowerCase()}"
			onclick={() => rename.start()}
			>{rename.shown}<span class="sr-only"> (edit {label.toLowerCase()})</span><Pencil
				size={14}
				aria-hidden="true"
			/></button
		>
	{/if}
	{#if rename.error}<span class="error" role="alert">{rename.error}</span>{/if}
</span>

<style>
	.rename {
		display: inline-flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: var(--space-1) var(--space-2);
		min-width: 0;
		max-width: 100%;
	}

	.text,
	.edit {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		padding: 0;
		border: 0;
		background: none;
		color: inherit;
		font: inherit;
		text-align: start;
		cursor: pointer;
	}

	.text {
		overflow-wrap: anywhere;
	}

	.text :global(svg),
	.edit {
		color: var(--color-text-secondary);
		opacity: 0.6;
	}

	.text:hover :global(svg),
	.text:focus-visible :global(svg),
	.edit:hover,
	.edit:focus-visible {
		color: var(--color-accent-text);
		opacity: 1;
	}

	.field {
		min-width: min(100%, 16rem);
		flex: 1;
		padding: var(--space-1) var(--space-2);
		border: 1px solid var(--color-accent-border);
		border-radius: var(--radius-sm);
		background: var(--color-surface);
		color: inherit;
		font: inherit;
	}

	.error {
		flex-basis: 100%;
		color: var(--color-danger-text);
		font-size: var(--text-sm);
		font-weight: var(--weight-regular);
	}
</style>
