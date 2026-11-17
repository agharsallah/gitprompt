<script lang="ts">
	import { goto } from '$app/navigation';
	import { api } from '#lib/api/client.ts';
	import type { FindResults } from '#lib/api/types.ts';
	import { Resource } from '#lib/async/resource.svelte.ts';
	import { describeError } from '#lib/errors/describe.ts';
	import { actionsFor, perform, type ActionContext } from '#lib/palette/actions.ts';
	import { commands } from '#lib/palette/commands.ts';
	import { copyText } from '#lib/palette/copy.ts';
	import { intentOf, keyLabel, moveActive } from '#lib/palette/keys.ts';
	import type { PaletteState } from '#lib/palette/open.svelte.ts';
	import PaletteView from '#lib/palette/PaletteView.svelte';
	import { kindsFor, parseQuery } from '#lib/palette/query.ts';
	import { buildView, rowsOf } from '#lib/palette/rows.ts';
	import { READY, type PanelState } from '#lib/ui/panelState.ts';
	import { toasts } from '#lib/ui/toast.svelte.ts';

	/**
	 * ⌘K: find a prompt, session or project, or run a command. A modal dialog (the page behind it is
	 * inert and focus cannot leave it; Esc closes it and focus goes back where it was) around one
	 * combobox. Loaded only when first opened (layout/+layout.svelte).
	 */
	let { palette }: { palette: PaletteState } = $props();

	/** How long typing pauses before the words are searched. */
	const DEBOUNCE_MS = 120;
	const mac = typeof navigator !== 'undefined' && /Mac|iPhone|iPad/i.test(navigator.platform);

	let dialog = $state<HTMLDialogElement>();
	let input = $state<HTMLInputElement>();
	let query = $state('');
	let settled = $state('');
	let active = $state(0);
	let attempt = $state(0);

	// The words are searched once typing pauses; opening again fetches the recent items afresh.
	function search(): Promise<FindResults | null> {
		void palette.opened;
		void attempt;
		const { mode, text } = parseQuery(settled);
		const kinds = kindsFor(mode);
		return kinds.length ? api.find(text, kinds) : Promise.resolve(null);
	}
	const found = new Resource<FindResults | null>(search);

	$effect(() => {
		const typed = query;
		if (typed === settled) return;
		const timer = setTimeout(() => (settled = typed), DEBOUNCE_MS);
		return () => clearTimeout(timer);
	});

	const parsed = $derived(parseQuery(query));
	const view = $derived(
		buildView(parsed, found.value ?? undefined, settled === query && !found.loading, commands())
	);
	const rows = $derived(rowsOf(view.groups));
	const current = $derived(rows.length ? Math.min(active, rows.length - 1) : -1);
	const keys = $derived(
		(rows[current] ? actionsFor(rows[current].item.kind) : []).map((a) => ({
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

	const ctx: ActionContext = {
		close: () => palette.hide(),
		navigate: (href) => {
			palette.hide();
			return goto(href);
		},
		copy: (text) => {
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
		if (intent.type === 'move') active = moveActive(current, intent.delta, rows.length);
		else if (rows[current]) void perform(rows[current].item, intent.key, ctx);
	}

	// Open and close follow the shared state; each opening starts from an empty box.
	$effect(() => {
		if (!dialog) return;
		if (palette.open && !dialog.open) {
			query = settled = '';
			active = 0;
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
	class="palette"
	aria-label="Find or run a command"
	onclose={() => palette.hide()}
>
	<PaletteView
		bind:query
		bind:input
		{view}
		active={current}
		{status}
		{keys}
		queryText={parsed.text}
		{onkeydown}
		oninput={() => (active = 0)}
		onpick={(index) => rows[index] && void perform(rows[index].item, 'enter', ctx)}
		onhover={(index) => (active = index)}
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

	dialog.palette::backdrop {
		background: rgb(0 0 0 / 0.45);
	}
</style>
