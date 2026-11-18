/**
 * Hiding the values of some possible leaks, after you confirm: the state behind
 * HideValues.svelte, so everything that can go wrong is tested here (hide.svelte.test.ts).
 *
 * Hiding rewrites the stored text and cannot be undone, so the findings are first listed (masked,
 * with their kind and where they sit), and only a confirm sends `hidden` for each.
 */
import { api } from '#lib/api/client.ts';
import type { StatsQuery } from '#lib/api/query.ts';
import type { LeakOut, LeakPlace, Leaks, LeakSaved } from '#lib/api/types.ts';
import { messageOf } from '#lib/errors/describe.ts';
import { plural } from '#lib/shared/format.ts';

/** Where a finding sits, as a sentence says it ("in your prompt"). */
export const PLACE_WORDS: Record<LeakPlace, string> = {
	prompt: 'your prompt',
	answer: 'the answer',
	tool_input: 'a tool’s input',
	tool_output: 'a tool’s output'
};

/** The open findings among `leaks` that `ids` name, in the order of `ids`. */
export function pickFindings(leaks: Leaks, ids: readonly string[]): LeakOut[] {
	const byId = new Map(leaks.open.map((leak) => [leak.id, leak]));
	return ids.flatMap((id) => byId.get(id) ?? []);
}

/** "Hidden 2 values". */
export const hiddenText = (n: number) => `Hidden ${plural(n, 'value')}`;

export type HideOptions = {
	/** The findings to hide. */
	findings: readonly string[];
	/** GET /api/leaks (over the range the findings were found in). */
	load: () => Promise<Leaks>;
	/** PUT /api/leaks/{id} with `hidden`. */
	hide: (id: string) => Promise<LeakSaved>;
};

export type HidePhase = 'loading' | 'confirm' | 'saving' | 'done' | 'error';

export class HideValues {
	phase = $state<HidePhase>('loading');
	/** What will be hidden: each finding masked, with its kind and place. */
	found = $state<LeakOut[]>([]);
	/** Why loading or hiding failed; empty otherwise. */
	error = $state('');
	/** How many values were hidden (also after a partial failure). */
	hidden = $state(0);
	#options: HideOptions;

	constructor(options: HideOptions) {
		this.#options = options;
	}

	/** Fetch what the findings are; none left open (hidden meanwhile) is an error to say. */
	async load(): Promise<void> {
		this.phase = 'loading';
		this.error = '';
		try {
			this.found = pickFindings(await this.#options.load(), this.#options.findings);
		} catch (e) {
			return this.#fail(`Could not read the possible leaks: ${messageOf(e)}`);
		}
		if (!this.found.length) return this.#fail('These values are no longer open: nothing to hide.');
		this.phase = 'confirm';
	}

	/** Hide every listed value, one after the other; stops at the first that fails. */
	async confirm(): Promise<number> {
		if (this.phase !== 'confirm') return 0;
		this.phase = 'saving';
		this.hidden = 0;
		for (const leak of this.found) {
			try {
				await this.#options.hide(leak.id);
			} catch (e) {
				this.#fail(`Could not hide ${leak.masked}: ${messageOf(e)}`);
				return this.hidden;
			}
			this.hidden += 1;
		}
		this.phase = 'done';
		return this.hidden;
	}

	#fail(message: string): void {
		this.error = message;
		this.phase = 'error';
	}
}

/** The flow over the daemon: the board of the range `query` keeps, and `hidden` per finding. */
export function hideWithApi(findings: readonly string[], query: StatsQuery = {}): HideValues {
	return new HideValues({
		findings,
		load: () => api.leaks(query),
		hide: (id) => api.setLeak(id, 'hidden')
	});
}
