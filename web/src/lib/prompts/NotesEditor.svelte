<script lang="ts">
	import { resolve } from '$app/paths';
	import type { Annotation } from '#lib/api/types.ts';
	import Chip from '#lib/ui/Chip.svelte';

	/**
	 * Tags and a note on a prompt. A tag is added with Enter or a comma, removed with its button
	 * (or Backspace in the empty field); the note is saved when you leave the field.
	 */
	let {
		value,
		save,
		saved = false
	}: { value: Annotation; save: (patch: Partial<Annotation>) => void; saved?: boolean } = $props();

	const id = $props.id();
	let draft = $state('');

	function add() {
		const fresh = draft
			.split(',')
			.map((t) => t.trim().replace(/^#/, ''))
			.filter((t) => t && !value.tags.includes(t));
		draft = '';
		if (fresh.length) save({ tags: [...value.tags, ...fresh] });
	}

	function key(event: KeyboardEvent) {
		if (event.key === 'Enter' || event.key === ',') {
			event.preventDefault();
			add();
		} else if (event.key === 'Backspace' && !draft && value.tags.length) {
			save({ tags: value.tags.slice(0, -1) });
		}
	}
</script>

<div class="notes">
	<div class="tags">
		<label for="{id}-tag" class="label">Tags</label>
		<div class="tag-box">
			{#each value.tags as tag (tag)}
				<Chip
					href="{resolve('/prompts')}?tag={encodeURIComponent(tag)}"
					onremove={() => save({ tags: value.tags.filter((t) => t !== tag) })}
					removeLabel="Remove tag {tag}"
				>
					#{tag}
				</Chip>
			{/each}
			<input
				id="{id}-tag"
				bind:value={draft}
				onkeydown={key}
				onblur={add}
				placeholder={value.tags.length ? 'Add another' : 'Add a tag, e.g. refactor'}
			/>
		</div>
	</div>
	<div class="note">
		<label for="{id}-note" class="label">
			Note
			{#if saved}<span class="saved" role="status">Saved</span>{/if}
		</label>
		<textarea
			id="{id}-note"
			rows="2"
			value={value.note}
			placeholder="What made this prompt work, or not?"
			onchange={(e) => save({ note: e.currentTarget.value })}
		></textarea>
	</div>
</div>

<style>
	.notes {
		display: grid;
		gap: var(--space-3);
	}

	.label {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		margin-bottom: var(--space-1);
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
		font-weight: var(--weight-medium);
	}

	.tag-box {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1);
		min-height: var(--control-md);
		padding: var(--space-1) var(--space-2);
		border: var(--border-hairline) solid var(--color-border-control);
		border-radius: var(--radius-md);
		background: var(--color-surface-raised);
	}

	.tag-box:focus-within,
	textarea:focus {
		border-color: var(--color-accent);
		box-shadow: 0 0 0 3px color-mix(in srgb, var(--color-accent) 20%, transparent);
	}

	input {
		flex: 1;
		min-width: 8rem;
		height: 1.5rem;
		border: 0;
		background: none;
		outline: none;
	}

	input:focus-visible {
		box-shadow: none;
	}

	textarea {
		display: block;
		width: 100%;
		padding: var(--space-2) var(--space-3);
		border: var(--border-hairline) solid var(--color-border-control);
		border-radius: var(--radius-md);
		background: var(--color-surface-raised);
		line-height: var(--leading-normal);
		resize: vertical;
		outline: none;
	}

	textarea:focus-visible {
		box-shadow: 0 0 0 3px color-mix(in srgb, var(--color-accent) 20%, transparent);
	}

	input::placeholder,
	textarea::placeholder {
		color: var(--color-text-muted);
	}

	.saved {
		color: var(--color-success-text);
		font-size: var(--text-xs);
		font-weight: var(--weight-regular);
	}
</style>
