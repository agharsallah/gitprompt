/**
 * A short message that confirms something you did and goes away by itself ("Copied · 1,240
 * characters"). One shared list, drawn once by `ToastHost` in the layout. For something rare and
 * deliberate; a frequent action (copy on a row) confirms in place instead.
 */
export type Toast = { id: number; message: string; tone: 'info' | 'error' };

/** How long a toast stays, unless the caller says otherwise. */
export const TOAST_MS = 4000;

export class Toasts {
	items = $state<Toast[]>([]);
	#next = 1;

	show(
		message: string,
		{ tone = 'info', ms = TOAST_MS }: { tone?: Toast['tone']; ms?: number } = {}
	) {
		const id = this.#next++;
		this.items = [...this.items, { id, message, tone }];
		setTimeout(() => this.dismiss(id), ms);
	}

	dismiss(id: number) {
		this.items = this.items.filter((t) => t.id !== id);
	}
}

export const toasts = new Toasts();
