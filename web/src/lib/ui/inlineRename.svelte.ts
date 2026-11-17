/**
 * The state of an inline rename (lib/ui/InlineRename.svelte): view, edit, save. The component only
 * draws it, so everything that can go wrong is tested here (inlineRename.state.test.ts).
 *
 * A save is optimistic: the new text shows at once and goes back to the old one if the save fails,
 * with the reason. Emptying the text goes back to the default (`fallback`) when there is one.
 */
import { messageOf } from '#lib/errors/describe.ts';

export type RenameOptions = {
	/** What is shown now, as the owner knows it (read again whenever it changes). */
	value: () => string;
	/** What an emptied text goes back to; without it, emptying the text cancels. */
	fallback?: () => string | undefined;
	/** Stores `next` (`null`: back to the default); may answer the text as stored. */
	save: (next: string | null) => Promise<string | null | void>;
	/** What the saved thing is called in an error ("the session name"). */
	what?: () => string | undefined;
};

/** The text of a draft: surrounding space cut, runs of white space made one space. */
export const cleaned = (draft: string) => draft.trim().replace(/\s+/g, ' ');

export class Rename {
	editing = $state(false);
	/** The text in the field while editing (bound to the input). */
	draft = $state('');
	saving = $state(false);
	/** Why the last save failed; empty otherwise. */
	error = $state('');
	#override = $state<string | null>(null);
	#options: RenameOptions;

	constructor(options: RenameOptions) {
		this.#options = options;
	}

	/** The text to show: the optimistic one while it holds, else the owner's. */
	get shown(): string {
		return this.#override ?? this.#options.value();
	}

	/** The owner's value changed: it wins over what was typed before, unless a save is running. */
	settle(): void {
		if (!this.saving) this.#override = null;
	}

	start(): void {
		if (this.saving || this.editing) return;
		this.draft = this.shown;
		this.error = '';
		this.editing = true;
	}

	cancel(): void {
		this.editing = false;
		this.error = '';
	}

	/** Save the draft (Enter, or leaving the field); does nothing when it changes nothing. */
	async commit(): Promise<void> {
		if (!this.editing) return;
		this.editing = false;
		const typed = cleaned(this.draft);
		const target = typed === '' ? null : typed;
		const next = target ?? this.#options.fallback?.();
		if (next === undefined || next === this.shown) return;
		const before = this.#override;
		this.#override = next;
		this.saving = true;
		this.error = '';
		try {
			const stored = await this.#options.save(target);
			this.#override = stored ?? next;
		} catch (e) {
			this.#override = before;
			this.error = `Could not save ${this.#options.what?.() ?? 'the change'}: ${messageOf(e)}`;
		} finally {
			this.saving = false;
		}
	}
}
