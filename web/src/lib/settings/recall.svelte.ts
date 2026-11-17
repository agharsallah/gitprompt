import type { Platform } from '#lib/platform/types.ts';

/** What Settings needs to read and save the stored shortcut. */
export type ShortcutStore = {
	value: () => string;
	/** Save it; false when saving failed (the editor shows why). */
	save: (shortcut: string) => Promise<boolean>;
};

/**
 * Choosing the recall shortcut. The app is asked first: a shortcut another app holds is refused
 * there, shown here, and never saved. Once the app holds the new one it is saved, and when saving
 * fails the old one is taken back, so what runs is always what is stored.
 */
export class RecallShortcut {
	/** Why the shortcut is not held: set at start, or by the last refusal; null when all is well. */
	problem = $state<string | null>(null);
	/** Waiting for a key press. */
	recording = $state(false);
	#store: ShortcutStore;
	#native: Platform['recall'];

	constructor(store: ShortcutStore, native: Platform['recall']) {
		this.#store = store;
		this.#native = native;
	}

	/** What the app reported at start. */
	async load(): Promise<void> {
		this.problem = await this.#native.problem().catch(() => null);
	}

	async choose(shortcut: string): Promise<void> {
		this.recording = false;
		const previous = this.#store.value();
		const result = await this.#native.register(shortcut);
		if (result.kind === 'refused') {
			this.problem = result.reason;
			return;
		}
		if (await this.#store.save(shortcut)) {
			this.problem = null;
			return;
		}
		await this.#native.register(previous);
	}
}
