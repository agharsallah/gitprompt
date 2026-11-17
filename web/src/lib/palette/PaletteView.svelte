<script lang="ts">
	import {
		ArrowRight,
		FolderGit2,
		MessagesSquare,
		Search,
		SquareTerminal,
		TextQuote
	} from '@lucide/svelte';
	import type { Component } from 'svelte';
	import type { Attachment } from 'svelte/attachments';
	import type { Kind } from '#lib/palette/actions.ts';
	import { firstIndexes, type View } from '#lib/palette/rows.ts';
	import PanelState from '#lib/ui/PanelState.svelte';
	import type { PanelState as State } from '#lib/ui/panelState.ts';
	import RelativeTime from '#lib/ui/RelativeTime.svelte';

	/**
	 * What the palette draws, from plain props: the box, the grouped rows (a combobox over a
	 * listbox: focus stays in the box and `active` is the highlighted option) and the keys for the
	 * highlighted row. All behaviour lives in Palette.svelte and the modules next to it.
	 */
	let {
		query = $bindable(''),
		view,
		active,
		status,
		keys,
		queryText,
		input = $bindable(),
		onkeydown,
		oninput,
		onpick,
		onhover
	}: {
		query?: string;
		view: View;
		/** Index of the highlighted row among all rows, -1 for none. */
		active: number;
		/** Loading or failed; `ready` when the rows can be shown. */
		status: State;
		/** The keys of the highlighted row, as written on a keycap with what they do. */
		keys: { key: string; label: string }[];
		/** The words typed after any prefix, for the empty and no-match lines. */
		queryText: string;
		input?: HTMLInputElement;
		onkeydown: (event: KeyboardEvent) => void;
		oninput: () => void;
		onpick: (index: number) => void;
		onhover: (index: number) => void;
	} = $props();

	const GLYPHS: Record<Kind, Component> = {
		prompt: TextQuote,
		session: MessagesSquare,
		project: FolderGit2,
		command: SquareTerminal,
		page: ArrowRight
	};

	const starts = $derived(firstIndexes(view.groups));
	/**
	 * The mouse, for the whole list at once (the options are never focused: the box keeps focus, so
	 * the keyboard is handled there): moving over a row highlights it, a click picks it.
	 */
	const pointer: Attachment<HTMLElement> = (list) => {
		const indexOf = (event: Event) => {
			const option =
				event.target instanceof Element ? event.target.closest<HTMLElement>('[data-index]') : null;
			return option ? Number(option.dataset.index) : undefined;
		};
		const move = (event: Event) => {
			const index = indexOf(event);
			if (index !== undefined) onhover(index);
		};
		const click = (event: Event) => {
			const index = indexOf(event);
			if (index !== undefined) onpick(index);
		};
		list.addEventListener('pointermove', move);
		list.addEventListener('click', click);
		return () => {
			list.removeEventListener('pointermove', move);
			list.removeEventListener('click', click);
		};
	};
	const hasRows = $derived(view.groups.length > 0);
	const optionId = (i: number) => `palette-option-${i}`;
</script>

<div class="palette">
	<div class="search">
		<Search size={16} aria-hidden="true" />
		<input
			bind:this={input}
			bind:value={query}
			type="text"
			role="combobox"
			aria-label="Find a prompt, session or project. Start with > for commands."
			aria-autocomplete="list"
			aria-expanded={hasRows}
			aria-controls={hasRows ? 'palette-list' : undefined}
			aria-activedescendant={hasRows && active >= 0 ? optionId(active) : undefined}
			placeholder="Find anything, or type > for commands"
			autocomplete="off"
			autocapitalize="off"
			spellcheck="false"
			{onkeydown}
			{oninput}
		/>
		<kbd>esc</kbd>
	</div>

	<div class="body">
		<PanelState state={status}>
			{#if view.noMatch}
				<p class="note">Nothing matches “{queryText}”.</p>
			{:else if !hasRows}
				<p class="note">
					{query.trimStart().startsWith('>')
						? `No command matches “${queryText}”.`
						: queryText
							? `Nothing matches “${queryText}”.`
							: 'Nothing captured yet.'}
				</p>
			{/if}
			{#if hasRows}
				<div id="palette-list" class="list" role="listbox" aria-label="Results" {@attach pointer}>
					{#each view.groups as group, g (group.kind)}
						{@const first = starts[g]}
						<div class="group" role="group" aria-labelledby="palette-group-{group.kind}">
							<p class="label" id="palette-group-{group.kind}">{group.label}</p>
							{#each group.rows as row, i (row.key)}
								{@const Glyph = GLYPHS[row.glyph]}
								<div
									id={optionId(first + i)}
									class={['row', first + i === active && 'active']}
									role="option"
									aria-selected={first + i === active}
									data-index={first + i}
								>
									<Glyph size={16} aria-hidden="true" />
									<span class="title"
										>{#each row.title as segment, s (s)}{#if segment.hit}<b>{segment.text}</b
												>{:else}{segment.text}{/if}{/each}</span
									>
									{#if row.context}<span class="context">{row.context}</span>{/if}
									{#if row.at}<span class="when"><RelativeTime iso={row.at} /></span>{/if}
								</div>
							{/each}
						</div>
					{/each}
				</div>
			{/if}
		</PanelState>
	</div>

	<footer>
		{#each keys as { key, label } (key)}
			<span><kbd>{key}</kbd> {label}</span>
		{/each}
		<span><kbd>↑↓</kbd> move</span>
		<span class="end"><kbd>esc</kbd> close</span>
	</footer>
</div>

<style>
	.palette {
		display: flex;
		flex-direction: column;
		max-height: min(32rem, 80dvh);
	}

	.search {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		padding: 0 var(--space-4);
		border-bottom: var(--border-hairline) solid var(--color-border);
		color: var(--color-text-muted);
	}

	input {
		flex: 1;
		height: var(--control-lg);
		min-width: 0;
		border: 0;
		background: none;
		color: var(--color-text);
		font-size: var(--text-md);
	}

	input:focus-visible {
		box-shadow: none;
		outline: none;
	}

	kbd {
		padding: 0 var(--space-1);
		border-radius: var(--radius-xs);
		background: var(--color-surface-sunken);
		box-shadow: inset 0 0 0 1px var(--color-border);
		color: var(--color-text-secondary);
		font-family: var(--font-mono);
		font-size: var(--text-xs);
	}

	.body {
		min-height: 0;
		padding: var(--space-2);
		overflow-y: auto;
	}

	.note {
		padding: var(--space-3) var(--space-2);
		color: var(--color-text-secondary);
		font-size: var(--text-sm);
	}

	.label {
		padding: var(--space-2) var(--space-2) var(--space-1);
		color: var(--color-text-muted);
		font-size: var(--text-xs);
		font-weight: var(--weight-medium);
	}

	.row {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		height: var(--control-md);
		padding: 0 var(--space-2);
		border-radius: var(--radius-md);
		color: var(--color-text-secondary);
		cursor: default;
	}

	.row.active {
		background: var(--color-surface-selected);
		color: var(--color-text);
		box-shadow: inset 2px 0 0 var(--color-accent);
	}

	.title {
		flex: 1 1 auto;
		min-width: 0;
		overflow: hidden;
		color: var(--color-text);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.title b {
		font-weight: var(--weight-bold);
	}

	.context,
	.when {
		flex: none;
		overflow: hidden;
		color: var(--color-text-muted);
		font-size: var(--text-sm);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.context {
		max-width: 12rem;
	}

	footer {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-4);
		padding: var(--space-2) var(--space-4);
		border-top: var(--border-hairline) solid var(--color-border);
		color: var(--color-text-muted);
		font-size: var(--text-xs);
	}

	.end {
		margin-left: auto;
	}
</style>
