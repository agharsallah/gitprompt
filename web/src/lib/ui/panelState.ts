/**
 * The states a panel, list or story can be in, as one closed union: a page picks one of these and
 * `PanelState.svelte` draws it, so no page invents a seventh way to say "nothing here".
 *
 * `ready` is the content itself (drawn by the caller's children, no frame added), so a page can
 * always pass a state, whatever it is. `fresh` also shows the content, with a live chip that says
 * new data arrived. Every other state stands in for the content.
 */

export type PanelBody = 'chart' | 'rows' | 'figures' | 'text';

/** What a panel says in place of its content: a title, a sentence, and the next step if there is one. */
export type Notice = {
	title: string;
	message?: string;
	next?: { label: string; href: string; title?: string };
};

export type PanelState =
	/** The content is there: children are drawn as they are. */
	| { kind: 'ready' }
	/** The data is on its way: a skeleton shaped like what comes. */
	| { kind: 'loading'; label?: string; body?: PanelBody; rows?: number; height?: string }
	/** Loaded, but there is nothing to show: what that means, and the next step if there is one. */
	| ({ kind: 'empty' } & Notice)
	/**
	 * No connected tool reports this: says which tool could, rather than showing zeros. The feature
	 * that knows the tools writes the notice (lib/sources/missing.ts).
	 */
	| ({ kind: 'unsupported' } & Notice)
	/** Loading failed: said as everywhere (lib/errors), with "Try again" when `retry` is given. */
	| { kind: 'error'; error: unknown; what?: string; retry?: () => unknown }
	/** The content, plus a live chip: `count` new `unit`s arrived since it was drawn. */
	| { kind: 'fresh'; count: number; unit?: string }
	/** Too little data to draw anything honest: the least it needs, and what there is so far. */
	| { kind: 'sparse'; minimum: number; unit?: string; have?: number };

export type PanelStateKind = PanelState['kind'];

/** Every kind, in the order the design page shows them. */
export const PANEL_STATE_KINDS = [
	'loading',
	'empty',
	'unsupported',
	'error',
	'fresh',
	'sparse',
	'ready'
] as const satisfies readonly PanelStateKind[];

export const READY: PanelState = { kind: 'ready' };

/** `READY` when there is something to show, else the empty state: the usual shape of a panel. */
export const readyOr = (has: unknown, empty: Extract<PanelState, { kind: 'empty' }>): PanelState =>
	has ? READY : empty;
