/**
 * Deciding about one possible leak, the state behind LeakActions.svelte (tested in
 * decide.svelte.test.ts): Hide asks once (HideValues, it rewrites the stored text), Not sensitive
 * is one click, Undo opens a dismissed finding again. Each decision is counted, then `onchanged`
 * runs: the page reloads its leaks, so the same value leaves every place it showed.
 */
import { api } from '#lib/api/client.ts';
import type { StatsQuery } from '#lib/api/query.ts';
import type { LeakDecision, LeakSaved } from '#lib/api/types.ts';
import { hideWithApi, type HideValues } from '#lib/leaks/hide.svelte.ts';
import { Task } from '#lib/shared/task.svelte.ts';
import { track } from '#lib/shared/usage.ts';
import type { LeakAction } from '#lib/shared/usage-actions.ts';

export type DecideOptions = {
	/** PUT /api/leaks/{id}: a decision, or null to open the finding again. */
	set: (id: string, state: LeakDecision | null) => Promise<LeakSaved>;
	/** The confirm-then-hide flow of these findings. */
	hideFlow: (findings: readonly string[]) => HideValues;
	count: (action: LeakAction) => void;
	/** The stored data changed: reload what shows the leaks. */
	onchanged?: () => void;
};

export class LeakDecider {
	/** The flow of Hide while it asks; null otherwise. */
	asking = $state<HideValues | null>(null);
	readonly task = new Task();
	#id: string;
	#options: DecideOptions;

	constructor(id: string, options: DecideOptions) {
		this.#id = id;
		this.#options = options;
	}

	/** Hide: list the value and ask (nothing is saved before the confirm). */
	ask(): void {
		this.asking = this.#options.hideFlow([this.#id]);
	}

	cancel(): void {
		this.asking = null;
	}

	/** The confirm of HideValues went through: `n` values hidden. */
	hidden(n: number): void {
		this.asking = null;
		if (!n) return;
		this.#options.count('leak.hide');
		this.#options.onchanged?.();
	}

	/** Not sensitive, at once: the daemon dismisses the same value wherever it appears. */
	dismiss(): Promise<boolean> {
		return this.#decide('not_sensitive', 'leak.dismiss');
	}

	/** A dismissed finding is open again. */
	undo(): Promise<boolean> {
		return this.#decide(null, 'leak.undo');
	}

	async #decide(state: LeakDecision | null, action: LeakAction): Promise<boolean> {
		const ok = await this.task.run(() => this.#options.set(this.#id, state));
		if (!ok) return false;
		this.#options.count(action);
		this.#options.onchanged?.();
		return true;
	}
}

/** The decisions over the daemon, the Hide flow reading the board of `query`. */
export function decideWithApi(
	id: string,
	{ query = {}, onchanged }: { query?: StatsQuery; onchanged?: () => void } = {}
): LeakDecider {
	return new LeakDecider(id, {
		set: (finding, state) => api.setLeak(finding, state),
		hideFlow: (findings) => hideWithApi(findings, query),
		count: track,
		onchanged
	});
}
