<script lang="ts">
	import { goto } from '$app/navigation';
	import { api } from '#lib/api/client.ts';
	import type { FindResults } from '#lib/api/types.ts';
	import { Resource } from '#lib/async/resource.svelte.ts';
	import { describeError } from '#lib/errors/describe.ts';
	import { perform, type ActionContext } from '#lib/palette/actions.ts';
	import { commands } from '#lib/palette/commands.ts';
	import { copyText } from '#lib/palette/copy.ts';
	import { highlightedIndex, intentOf, isMac, keyLabel, moveActive } from '#lib/palette/keys.ts';
	import {
		actionKeyIn,
		actionsIn,
		copyThenDone,
		kindsIn,
		parseFor,
		restrict,
		wordsFor,
		type PaletteMode
	} from '#lib/palette/mode.ts';
	import type { PaletteState } from '#lib/palette/open.svelte.ts';
	import PaletteView from '#lib/palette/PaletteView.svelte';
	import { buildView, rowsOf } from '#lib/palette/rows.ts';
	import { platform } from '#lib/platform/index.ts';
	import { READY, type PanelState } from '#lib/ui/panelState.ts';
	import { toasts } from '#lib/ui/toast.svelte.ts';

	/**
	 * ⌘K: find a prompt, session or project, or run a command. A modal dialog (the page behind it is
	 * inert and focus cannot leave it; Esc closes it and focus goes back where it was) around one
	 * combobox. Loaded only when first opened (layout/+layout.svelte).
	 *
	 * `mode="recall"` is the desktop recall window (palette/mode.ts): prompts only, ↵ copies, and
	 * the window hides whenever the dialog closes (Esc, or a copy that went through).
	 */
	let { palette, mode = 'full' }: { palette: PaletteState; mode?: PaletteMode } = $props();
	const recall = $derived(mode === 'recall');
	const words = $derived(wordsFor(mode));

	/** How long typing pauses before the words are searched. */
	const DEBOUNCE_MS = 120;
	const mac = isMac();

	let dialog = $state<HTMLDialogElement>();
	let input = $state<HTMLInputElement>();
	let query = $state('');
	let settled = $state('');
	/** The highlighted row, by its key: results that change under it do not move it to another row. */
	let highlighted = $state<string>();
	let attempt = $state(0);

	// The words are searched once typing pauses; opening again fetches the recent items afresh.
	function search(): Promise<FindResults | null> {
		void palette.opened;
		void attempt;
		const parsed = parseFor(mode, settled);
		const kinds = kindsIn(mode, parsed);
		return kinds.length ? api.find(parsed.text, kinds) : Promise.resolve(null);
	}
	const found = new Resource<FindResults | null>(search);

	$effect(() => {
		const typed = query;
		if (typed === settled) return;
		const timer = setTimeout(() => (settled = typed), DEBOUNCE_MS);
		return () => clearTimeout(timer);
	});

	const parsed = $derived(parseFor(mode, query));
	const view = $derived(
		restrict(
			mode,
			buildView(
				parsed,
				found.value ?? undefined,
				settled === query && !found.loading,
				recall ? [] : commands()
			)
		)
	);
	const rows = $derived(rowsOf(view.groups));
	const current = $derived(
		highlightedIndex(
			rows.map((row) => row.key),
			highlighted
		)
	);
	const keys = $derived(
		(rows[current] ? actionsIn(mode, rows[current].item.kind) : []).map((a) => ({
			key: keyLabel(a.key, mac),
			label: a.label
		}))
	);
	const status = $derived<PanelState>(
		found.failed
			? { kind: 'error', error: found.error, what: 'the search', retry: () => attempt++ }
			: found.loading && !found.ready
				? { kind: 'loading', label: 'Searching', body: 'rows', rows: 3 }
				: READY
	);

	const copyInRecall = copyThenDone(copyText, () => palette.hide());
	const ctx: ActionContext = {
		close: () => palette.hide(),
		navigate: (href) => {
			palette.hide();
			return goto(href);
		},
		// Recall keeps its window until the text is on the clipboard (closing it hides the window).
		copy: (text) => {
			if (recall) return copyInRecall(text);
			palette.hide();
			return copyText(text);
		},
		promptText: async (id) => {
			const prompt = await api.prompt(id);
			return prompt.typed_text ?? prompt.prompt ?? '';
		},
		fail: (error) => toasts.show(describeError(error, undefined, 'that').title, { tone: 'error' })
	};

	function onkeydown(event: KeyboardEvent) {
		const intent = intentOf(event);
		if (!intent) return;
		event.preventDefault();
		if (intent.type === 'move')
			highlighted = rows[moveActive(current, intent.delta, rows.length)]?.key;
		else if (rows[current]) void perform(rows[current].item, actionKeyIn(mode, intent.key), ctx);
	}

	// Open and close follow the shared state; each opening starts from an empty box.
	$effect(() => {
		if (!dialog) return;
		if (palette.open && !dialog.open) {
			query = settled = '';
			highlighted = undefined;
			dialog.showModal();
			input?.focus();
		} else if (!palette.open && dialog.open) dialog.close();
	});

	$effect(() => {
		if (current >= 0)
			document.getElementById(`palette-option-${current}`)?.scrollIntoView({ block: 'nearest' });
	});
</script>

<dialog
	bind:this={dialog}
	class={['palette', recall && 'recall']}
	aria-label={words.dialog}
	onclose={() => {
		palette.hide();
		// The recall window has nothing else to show: closing the dialog is leaving (unless the
		// window lost focus first, which has already sent it away).
		if (recall && document.hasFocus()) void platform.hideWindow();
	}}
>
	<PaletteView
		bind:query
		bind:input
		{view}
		active={current}
		{status}
		{keys}
		queryText={parsed.text}
		commandMode={parsed.mode === 'commands'}
		{words}
		{onkeydown}
		oninput={() => (highlighted = undefined)}
		onpick={(index) => rows[index] && void perform(rows[index].item, 'enter', ctx)}
		onhover={(index) => (highlighted = rows[index]?.key)}
	/>
</dialog>

<style>
	dialog.palette {
		width: min(40rem, calc(100% - 2 * var(--space-4)));
		margin: 12dvh auto auto;
		padding: 0;
		border: var(--border-hairline) solid var(--color-border);
		border-radius: var(--radius-xl);
		background: var(--color-surface-raised);
		color: var(--color-text);
		box-shadow: var(--shadow-overlay);
	}

	/* The recall window is the dialog: it fills the window, with no page behind it. */
	dialog.palette.recall {
		--palette-max-height: 100dvh;
		width: 100%;
		max-width: none;
		height: 100dvh;
		margin: 0;
		border: 0;
		border-radius: 0;
		box-shadow: none;
	}

	dialog.palette::backdrop {
		background: rgb(0 0 0 / 0.45);
	}
</style>
