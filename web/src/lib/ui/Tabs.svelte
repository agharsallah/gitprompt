<script lang="ts" generics="T extends string">
	import type { Snippet } from 'svelte';
	import { tabKeyTarget } from '#lib/ui/tabs.ts';

	/** `hint`: what the tab holds, in a few words under its name. */
	type Tab = { id: T; label: string; count?: number | string; hint?: string; disabled?: boolean };

	/**
	 * WAI-ARIA tabs: one tab stop, arrow keys move between tabs (Home/End jump), activation follows
	 * focus. The panel snippet receives the selected tab's id.
	 */
	let {
		tabs,
		selected = $bindable(),
		label,
		panel,
		onselect
	}: {
		tabs: Tab[];
		selected: T;
		label: string;
		panel: Snippet<[T]>;
		/** Called when the user picks a tab (a click or an arrow key), not when `selected` is set. */
		onselect?: (id: T) => void;
	} = $props();

	function select(id: T) {
		if (id === selected) return;
		selected = id;
		onselect?.(id);
	}

	const id = $props.id();
	const buttons: HTMLButtonElement[] = $state([]);

	function move(event: KeyboardEvent, index: number) {
		const target = tabKeyTarget(event.key, index, tabs);
		if (target === null) return;
		event.preventDefault();
		select(tabs[target].id);
		buttons[target]?.focus();
	}
</script>

<div class="tabs">
	<div class="list" role="tablist" aria-label={label}>
		{#each tabs as tab, i (tab.id)}
			<button
				bind:this={buttons[i]}
				type="button"
				role="tab"
				id="{id}-tab-{tab.id}"
				aria-controls="{id}-panel"
				aria-selected={tab.id === selected}
				tabindex={tab.id === selected ? 0 : -1}
				disabled={tab.disabled}
				onclick={() => select(tab.id)}
				onkeydown={(e) => move(e, i)}
			>
				{#if tab.hint}
					<span class="named">
						<span class="label">{tab.label}</span>
						<span class="hint">{tab.hint}</span>
					</span>
				{:else}
					{tab.label}
				{/if}
				{#if tab.count !== undefined && tab.count !== 0}<span class="count num">{tab.count}</span
					>{/if}
			</button>
		{/each}
	</div>
	<div
		class="panel"
		role="tabpanel"
		id="{id}-panel"
		aria-labelledby="{id}-tab-{selected}"
		tabindex="-1"
	>
		{@render panel(selected)}
	</div>
</div>

<style>
	.list {
		display: flex;
		gap: var(--space-1);
		overflow-x: auto;
		border-bottom: var(--border-hairline) solid var(--color-border);
		scrollbar-width: none;
	}

	[role='tab'] {
		position: relative;
		display: inline-flex;
		align-items: center;
		gap: var(--space-1_5);
		height: var(--control-lg);
		padding: 0 var(--space-3);
		color: var(--color-text-secondary);
		font-size: var(--text-md);
		font-weight: var(--weight-medium);
		white-space: nowrap;
	}

	[role='tab']:hover:not(:disabled) {
		color: var(--color-text);
	}

	[role='tab']::after {
		content: '';
		position: absolute;
		right: var(--space-2);
		bottom: -1px;
		left: var(--space-2);
		height: 2px;
		border-radius: 2px 2px 0 0;
		background: transparent;
		transition: background-color var(--duration-fast) var(--ease-out);
	}

	[role='tab'][aria-selected='true'] {
		color: var(--color-text);
	}

	[role='tab'][aria-selected='true']::after {
		background: var(--color-accent);
	}

	[role='tab']:focus-visible {
		box-shadow: inset var(--focus-ring);
	}

	[role='tab']:disabled {
		color: var(--color-text-disabled);
	}

	.named {
		display: grid;
		justify-items: start;
		padding: var(--space-2) 0;
		line-height: var(--leading-tight);
		text-align: start;
	}

	[role='tab']:has(.named) {
		height: auto;
	}

	.hint {
		color: var(--color-text-muted);
		font-size: var(--text-xs);
		font-weight: var(--weight-regular);
	}

	.count {
		min-width: 1.25rem;
		padding: 1px var(--space-1_5);
		border-radius: var(--radius-full);
		background: var(--color-surface-sunken);
		color: var(--color-text-muted);
		font-size: var(--text-2xs);
		text-align: center;
	}

	.panel {
		padding-top: var(--space-5);
	}

	.panel:focus-visible {
		box-shadow: none;
	}
</style>
